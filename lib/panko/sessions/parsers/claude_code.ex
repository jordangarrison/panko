defmodule Panko.Sessions.Parsers.ClaudeCode do
  @moduledoc """
  Parser for Claude Code JSONL session files.

  Reads JSONL files line-by-line and converts them to the session
  attributes format expected by Session's import actions.
  """

  @behaviour Panko.Sessions.Parsers.Parser

  @impl true
  def source_type, do: :claude_code

  @impl true
  def can_parse?(path), do: String.ends_with?(path, ".jsonl")

  @impl true
  def parse(path) do
    case File.read(path) do
      {:ok, content} ->
        lines =
          content
          |> String.split("\n", trim: true)
          |> Enum.flat_map(fn line ->
            case Jason.decode(line) do
              {:ok, parsed} -> [parsed]
              {:error, _} -> []
            end
          end)

        session_id = extract_session_id(lines)
        project = extract_project(lines)
        started_at = extract_started_at(lines)
        title = extract_title(lines)

        {blocks, sub_agents} = extract_blocks_and_agents(lines)

        {:ok,
         %{
           external_id: session_id,
           source_type: :claude_code,
           source_path: path,
           project: project,
           title: title,
           started_at: started_at,
           blocks: blocks,
           sub_agents: sub_agents
         }}

      {:error, reason} ->
        {:error, {:file_read_error, reason}}
    end
  end

  defp extract_session_id(lines) do
    lines
    |> Enum.find_value(fn line -> line["sessionId"] end)
    |> Kernel.||("unknown")
  end

  defp extract_project(lines) do
    Enum.find_value(lines, fn line -> line["cwd"] end)
  end

  defp extract_started_at(lines) do
    lines
    |> Enum.find_value(fn line -> line["timestamp"] end)
    |> parse_timestamp()
    |> Kernel.||(DateTime.utc_now())
  end

  defp extract_title(lines) do
    case Enum.find(lines, fn line ->
           line["type"] == "user" && is_binary(get_in(line, ["message", "content"]))
         end) do
      nil ->
        nil

      line ->
        line
        |> get_in(["message", "content"])
        |> String.slice(0, 200)
    end
  end

  defp extract_blocks_and_agents(lines) do
    {blocks_rev, agents_rev, _pos} =
      lines
      |> Enum.filter(&(&1["type"] in ["user", "assistant"]))
      |> Enum.reduce({[], [], 0}, fn line, {blocks, agents, pos} ->
        {new_blocks, new_agents, new_pos} = process_line(line, pos)
        {Enum.reverse(new_blocks) ++ blocks, Enum.reverse(new_agents) ++ agents, new_pos}
      end)

    {Enum.reverse(blocks_rev), Enum.reverse(agents_rev)}
  end

  defp process_line(%{"type" => "user", "message" => message} = line, pos) do
    content = message["content"]
    timestamp = parse_timestamp(line["timestamp"])

    cond do
      is_binary(content) ->
        block = %{
          position: pos,
          block_type: :user_prompt,
          content: content,
          metadata: nil,
          timestamp: timestamp
        }

        {[block], [], pos + 1}

      is_list(content) ->
        # Tool results — skip as standalone blocks
        {[], [], pos}

      true ->
        {[], [], pos}
    end
  end

  defp process_line(%{"type" => "assistant", "message" => message} = line, pos) do
    content_parts = message["content"] || []
    timestamp = parse_timestamp(line["timestamp"])

    {blocks_rev, agents_rev, next_pos} =
      Enum.reduce(content_parts, {[], [], pos}, fn part, {blks, agts, p} ->
        case part["type"] do
          "text" ->
            block = %{
              position: p,
              block_type: :assistant_response,
              content: part["text"],
              metadata: nil,
              timestamp: timestamp
            }

            {[block | blks], agts, p + 1}

          "tool_use" ->
            {tool_block, maybe_agent} = process_tool_use(part, p, timestamp)
            {[tool_block | blks], Enum.reverse(maybe_agent) ++ agts, p + 1}

          "thinking" ->
            block = %{
              position: p,
              block_type: :thinking,
              content: part["thinking"],
              metadata: nil,
              timestamp: timestamp
            }

            {[block | blks], agts, p + 1}

          _ ->
            {blks, agts, p}
        end
      end)

    {Enum.reverse(blocks_rev), Enum.reverse(agents_rev), next_pos}
  end

  defp process_line(_line, pos), do: {[], [], pos}

  defp process_tool_use(part, pos, timestamp) do
    tool_name = part["name"]
    input = part["input"]
    tool_id = part["id"]

    {block_type, metadata} = categorize_tool(tool_name, input)

    block = %{
      position: pos,
      block_type: block_type,
      content: nil,
      metadata: metadata,
      timestamp: timestamp
    }

    agents =
      if block_type == :sub_agent_spawn do
        [
          %{
            external_id: tool_id || "unknown",
            agent_type: input["subagent_type"] || input["type"] || "unknown",
            description: input["description"] || "",
            prompt: input["prompt"] || "",
            status: :running,
            spawned_at: timestamp
          }
        ]
      else
        []
      end

    {block, agents}
  end

  defp categorize_tool("Write", input) do
    {:file_edit, %{"name" => "Write", "path" => input["file_path"], "input" => input}}
  end

  defp categorize_tool("Edit", input) do
    {:file_edit, %{"name" => "Edit", "path" => input["file_path"], "input" => input}}
  end

  defp categorize_tool("Agent", input) do
    {:sub_agent_spawn,
     %{
       "name" => "Agent",
       "agent_type" => input["subagent_type"] || input["type"],
       "description" => input["description"],
       "input" => input
     }}
  end

  defp categorize_tool(name, input) do
    {:tool_call, %{"name" => name, "input" => input}}
  end

  defp parse_timestamp(nil), do: nil

  defp parse_timestamp(ts) when is_binary(ts) do
    case DateTime.from_iso8601(ts) do
      {:ok, dt, _offset} -> DateTime.truncate(dt, :second)
      _ -> nil
    end
  end
end

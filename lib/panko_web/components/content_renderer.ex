defmodule PankoWeb.Components.ContentRenderer do
  @moduledoc """
  Parses assistant response content containing inline XML-like tags
  into structured segments for rendering.

  Handles tags emitted by Claude Code sessions:
  - `<command-name>...</command-name>` - command name badge
  - `<command>...</command>` - command content block
  - `<command-message>...</command-message>` - command message
  - `<command-args>...</command-args>` - command arguments
  - `<local-command-stdout>...</local-command-stdout>` - command output
  - `<local-command-caveat>...</local-command-caveat>` - caveat notice
  - `<system-reminder>...</system-reminder>` - stripped entirely
  """

  @doc """
  Parses content string into a list of tagged segments.

  Returns a list of `{:markdown, text}` or `{:tag_type, content}` tuples.

  ## Examples

      iex> parse_content("hello<command-name>ls</command-name>world")
      [{:markdown, "hello"}, {:command_name, "ls"}, {:markdown, "world"}]

      iex> parse_content(nil)
      []
  """
  @spec parse_content(String.t() | nil) :: [{atom(), String.t()}]
  def parse_content(nil), do: []
  def parse_content(""), do: []

  def parse_content(content) when is_binary(content) do
    combined_pattern()
    |> Regex.split(content, include_captures: true)
    |> Enum.flat_map(&classify_segment/1)
    |> Enum.reject(fn
      {:markdown, text} -> String.trim(text) == ""
      _ -> false
    end)
  end

  @doc """
  Renders a markdown string to safe HTML using Earmark.

  Returns a `Phoenix.HTML.safe()` value suitable for direct use in HEEx templates.
  """
  @spec render_markdown(String.t()) :: Phoenix.HTML.safe()
  def render_markdown(text) when is_binary(text) do
    text
    |> Earmark.as_html!(compact_output: true)
    |> Phoenix.HTML.raw()
  end

  def render_markdown(_), do: Phoenix.HTML.raw("")

  # -- Private helpers --

  # Tag patterns ordered so more specific tags match before shorter ones.
  # {regex, atom_type} where :strip means discard entirely.
  defp tag_patterns do
    [
      {~r/<system-reminder>.*?<\/system-reminder>/s, :strip},
      {~r/<command-name>(.*?)<\/command-name>/s, :command_name},
      {~r/<command-message>(.*?)<\/command-message>/s, :command_message},
      {~r/<command-args>(.*?)<\/command-args>/s, :command_args},
      {~r/<local-command-stdout>(.*?)<\/local-command-stdout>/s, :command_stdout},
      {~r/<local-command-caveat>(.*?)<\/local-command-caveat>/s, :command_caveat},
      {~r/<command>(.*?)<\/command>/s, :command}
    ]
  end

  defp combined_pattern do
    tag_patterns()
    |> Enum.map(fn {regex, _type} -> regex.source end)
    |> Enum.join("|")
    |> Regex.compile!("s")
  end

  defp classify_segment(""), do: []

  defp classify_segment(text) do
    case find_matching_tag(text) do
      {:strip, _} -> []
      {type, content} -> [{type, content}]
      nil -> [{:markdown, text}]
    end
  end

  defp find_matching_tag(text) do
    Enum.find_value(tag_patterns(), fn {regex, type} ->
      case Regex.run(regex, text) do
        [_full] when type == :strip -> {:strip, nil}
        [_full, captured] -> {type, captured}
        _ -> nil
      end
    end)
  end
end

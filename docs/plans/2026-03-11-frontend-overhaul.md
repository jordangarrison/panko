# Frontend Overhaul Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Bring the Elixir/Phoenix frontend to feature parity with the Rust version — project accordion index with global search, tag-aware content rendering, and block UI improvements.

**Architecture:** Replace flat session list with a project-grouped accordion landing page. Add Earmark for markdown rendering. Build a content parser module that interprets inline XML-like tags (`<command>`, `<output>`, etc.) into styled segments. Enhance all block components with timestamps, copy buttons, and large-output collapsing.

**Tech Stack:** Phoenix LiveView, Tailwind CSS + daisyUI, Earmark (markdown), HEEx components

---

### Task 1: Add Earmark Dependency

**Files:**
- Modify: `mix.exs` (deps section)

**Step 1: Add earmark to deps**

In `mix.exs`, add to the `defp deps` list:

```elixir
{:earmark, "~> 1.4"}
```

**Step 2: Fetch dependencies**

Run: `mix deps.get`
Expected: Earmark downloaded successfully

**Step 3: Verify compilation**

Run: `mix compile --warnings-as-errors`
Expected: Compiles cleanly

**Step 4: Commit**

```bash
git add mix.exs mix.lock
git commit -m "feat: add earmark dependency for markdown rendering"
```

---

### Task 2: Build Content Renderer Module

**Files:**
- Create: `lib/panko_web/components/content_renderer.ex`
- Create: `test/panko_web/components/content_renderer_test.exs`

This module parses assistant response content that may contain inline XML-like tags and renders them into safe HTML segments. It splits content into segments: plain text (rendered as markdown via Earmark) and tagged blocks (rendered with semantic styling).

**Step 1: Write failing tests**

```elixir
defmodule PankoWeb.Components.ContentRendererTest do
  use ExUnit.Case, async: true

  alias PankoWeb.Components.ContentRenderer

  describe "parse_content/1" do
    test "plain text passes through as markdown" do
      segments = ContentRenderer.parse_content("Hello **world**")
      assert [{:markdown, "Hello **world**"}] = segments
    end

    test "extracts command-name tags" do
      input = "some text<command-name>foo</command-name>more text"
      segments = ContentRenderer.parse_content(input)

      assert [
               {:markdown, "some text"},
               {:command_name, "foo"},
               {:markdown, "more text"}
             ] = segments
    end

    test "extracts command blocks" do
      input = "before<command>do something</command>after"
      segments = ContentRenderer.parse_content(input)

      assert [
               {:markdown, "before"},
               {:command, "do something"},
               {:markdown, "after"}
             ] = segments
    end

    test "extracts command-message blocks" do
      input = "text<command-message>msg here</command-message>end"
      segments = ContentRenderer.parse_content(input)

      assert [
               {:markdown, "text"},
               {:command_message, "msg here"},
               {:markdown, "end"}
             ] = segments
    end

    test "extracts local-command-stdout blocks" do
      input = "before<local-command-stdout>output</local-command-stdout>after"
      segments = ContentRenderer.parse_content(input)

      assert [
               {:markdown, "before"},
               {:command_stdout, "output"},
               {:markdown, "after"}
             ] = segments
    end

    test "extracts local-command-caveat blocks" do
      input = "text<local-command-caveat>caveat text</local-command-caveat>rest"
      segments = ContentRenderer.parse_content(input)

      assert [
               {:markdown, "text"},
               {:command_caveat, "caveat text"},
               {:markdown, "rest"}
             ] = segments
    end

    test "extracts command-args blocks" do
      input = "text<command-args>--flag val</command-args>rest"
      segments = ContentRenderer.parse_content(input)

      assert [
               {:markdown, "text"},
               {:command_args, "--flag val"},
               {:markdown, "rest"}
             ] = segments
    end

    test "handles multiple tags in one string" do
      input = "start<command-name>ls</command-name> ran <command>ls -la</command>end"
      segments = ContentRenderer.parse_content(input)

      assert [
               {:markdown, "start"},
               {:command_name, "ls"},
               {:markdown, " ran "},
               {:command, "ls -la"},
               {:markdown, "end"}
             ] = segments
    end

    test "strips system-reminder tags entirely" do
      input = "visible<system-reminder>secret stuff</system-reminder>also visible"
      segments = ContentRenderer.parse_content(input)

      assert [
               {:markdown, "visible"},
               {:markdown, "also visible"}
             ] = segments
    end

    test "handles nil content" do
      assert [] = ContentRenderer.parse_content(nil)
    end

    test "handles empty content" do
      assert [] = ContentRenderer.parse_content("")
    end

    test "filters out empty markdown segments" do
      input = "<command-name>foo</command-name>"
      segments = ContentRenderer.parse_content(input)
      assert [{:command_name, "foo"}] = segments
    end
  end

  describe "render_markdown/1" do
    test "converts markdown to safe HTML" do
      result = ContentRenderer.render_markdown("Hello **world**")
      html = Phoenix.HTML.safe_to_string(result)
      assert html =~ "<strong>world</strong>"
    end

    test "handles code blocks" do
      result = ContentRenderer.render_markdown("```elixir\nIO.puts(\"hi\")\n```")
      html = Phoenix.HTML.safe_to_string(result)
      assert html =~ "<code"
    end
  end
end
```

**Step 2: Run tests to verify they fail**

Run: `mix test test/panko_web/components/content_renderer_test.exs`
Expected: FAIL — module not defined

**Step 3: Implement ContentRenderer**

```elixir
defmodule PankoWeb.Components.ContentRenderer do
  @moduledoc """
  Parses assistant response content containing inline XML-like tags
  into structured segments for rendering.

  Handles tags emitted by Claude Code sessions:
  - <command-name>...</command-name> → command name badge
  - <command>...</command> → command content block
  - <command-message>...</command-message> → command message
  - <command-args>...</command-args> → command arguments
  - <local-command-stdout>...</local-command-stdout> → command output
  - <local-command-caveat>...</local-command-caveat> → caveat notice
  - <system-reminder>...</system-reminder> → stripped entirely
  """

  @tag_patterns [
    {~r/<system-reminder>.*?<\/system-reminder>/s, :strip},
    {~r/<command-name>(.*?)<\/command-name>/s, :command_name},
    {~r/<command-message>(.*?)<\/command-message>/s, :command_message},
    {~r/<command-args>(.*?)<\/command-args>/s, :command_args},
    {~r/<local-command-stdout>(.*?)<\/local-command-stdout>/s, :command_stdout},
    {~r/<local-command-caveat>(.*?)<\/local-command-caveat>/s, :command_caveat},
    {~r/<command>(.*?)<\/command>/s, :command}
  ]

  @doc """
  Parses content string into a list of tagged segments.
  Returns a list of `{:markdown, text}` or `{:tag_type, content}` tuples.
  """
  def parse_content(nil), do: []
  def parse_content(""), do: []

  def parse_content(content) when is_binary(content) do
    # Build a combined regex that matches any known tag
    combined_pattern =
      @tag_patterns
      |> Enum.map(fn {regex, _type} -> regex.source end)
      |> Enum.join("|")
      |> Regex.compile!("s")

    # Split content by all tag patterns, keeping delimiters
    parts = Regex.split(combined_pattern, content, include_captures: true)

    parts
    |> Enum.flat_map(&classify_segment/1)
    |> Enum.reject(fn
      {:markdown, text} -> String.trim(text) == ""
      _ -> false
    end)
  end

  defp classify_segment(text) do
    case find_matching_tag(text) do
      {:strip, _} -> []
      {type, content} -> [{type, content}]
      nil -> [{:markdown, text}]
    end
  end

  defp find_matching_tag(text) do
    Enum.find_value(@tag_patterns, fn {regex, type} ->
      case Regex.run(regex, text) do
        [_full] when type == :strip -> {:strip, nil}
        [_full, captured] -> {type, captured}
        _ -> nil
      end
    end)
  end

  @doc """
  Renders a markdown string to safe HTML using Earmark.
  """
  def render_markdown(text) when is_binary(text) do
    text
    |> Earmark.as_html!(compact_output: true)
    |> Phoenix.HTML.raw()
  end

  def render_markdown(_), do: Phoenix.HTML.raw("")
end
```

**Step 4: Run tests**

Run: `mix test test/panko_web/components/content_renderer_test.exs`
Expected: All tests pass

**Step 5: Verify compilation**

Run: `mix compile --warnings-as-errors`
Expected: Clean compilation

**Step 6: Commit**

```bash
git add lib/panko_web/components/content_renderer.ex test/panko_web/components/content_renderer_test.exs
git commit -m "feat: add ContentRenderer for tag-aware markdown rendering"
```

---

### Task 3: Update AssistantResponse Block to Use ContentRenderer

**Files:**
- Modify: `lib/panko_web/components/blocks/assistant_response.ex`

**Step 1: Write the updated component**

Replace the entire `assistant_response.ex` with a version that uses ContentRenderer to parse and render segments:

```elixir
defmodule PankoWeb.Components.Blocks.AssistantResponse do
  use Phoenix.Component

  alias PankoWeb.Components.ContentRenderer

  attr :block, :map, required: true

  def render(assigns) do
    assigns = assign(assigns, :segments, ContentRenderer.parse_content(assigns.block.content))

    ~H"""
    <article class="block border border-base-300 rounded-lg mb-4 border-l-4 border-l-success overflow-hidden">
      <div class="flex items-center justify-between px-4 py-2 bg-base-200/50 border-b border-base-300">
        <span class="text-sm font-semibold text-success">Assistant</span>
        <time :if={@block.timestamp} class="text-xs text-base-content/50">
          {format_time(@block.timestamp)}
        </time>
      </div>
      <div class="px-4 py-3 prose prose-sm max-w-none dark:prose-invert">
        <.render_segment :for={segment <- @segments} segment={segment} />
      </div>
    </article>
    """
  end

  defp render_segment(%{segment: {:markdown, text}} = assigns) do
    assigns = assign(assigns, :html, ContentRenderer.render_markdown(text))

    ~H"""
    <div>{@html}</div>
    """
  end

  defp render_segment(%{segment: {:command_name, name}} = assigns) do
    assigns = assign(assigns, :name, name)

    ~H"""
    <span class="badge badge-neutral badge-sm font-mono mx-1">{@name}</span>
    """
  end

  defp render_segment(%{segment: {:command, content}} = assigns) do
    assigns = assign(assigns, :content, content)

    ~H"""
    <div class="bg-base-200 rounded-md px-3 py-2 my-2 font-mono text-sm border border-base-300">
      <div class="text-xs text-base-content/50 mb-1 font-sans">Command</div>
      <div class="whitespace-pre-wrap">{@content}</div>
    </div>
    """
  end

  defp render_segment(%{segment: {:command_message, content}} = assigns) do
    assigns = assign(assigns, :content, content)

    ~H"""
    <div class="bg-base-200 rounded-md px-3 py-2 my-2 text-sm border border-base-300">
      <div class="whitespace-pre-wrap">{@content}</div>
    </div>
    """
  end

  defp render_segment(%{segment: {:command_args, content}} = assigns) do
    assigns = assign(assigns, :content, content)

    ~H"""
    <code class="text-xs bg-base-300 px-1.5 py-0.5 rounded font-mono">{@content}</code>
    """
  end

  defp render_segment(%{segment: {:command_stdout, content}} = assigns) do
    assigns = assign(assigns, :content, content)

    ~H"""
    <pre class="bg-base-300/50 rounded-md px-3 py-2 my-2 text-xs overflow-x-auto border border-base-300"><code>{@content}</code></pre>
    """
  end

  defp render_segment(%{segment: {:command_caveat, content}} = assigns) do
    assigns = assign(assigns, :content, content)

    ~H"""
    <div class="text-xs text-base-content/40 italic my-1">{@content}</div>
    """
  end

  defp render_segment(assigns) do
    ~H""
  end

  defp format_time(nil), do: ""
  defp format_time(%DateTime{} = dt), do: Calendar.strftime(dt, "%H:%M:%S")
end
```

**Step 2: Verify compilation**

Run: `mix compile --warnings-as-errors`
Expected: Clean

**Step 3: Run all tests**

Run: `mix test`
Expected: All pass

**Step 4: Commit**

```bash
git add lib/panko_web/components/blocks/assistant_response.ex
git commit -m "feat: render assistant responses with tag-aware markdown"
```

---

### Task 4: Update All Block Components — Timestamps, Borders, Structure

Update UserPrompt, ToolCall, Thinking, FileEdit, SubAgentSpawn to match the Rust version's article-based layout with colored left borders, timestamps on all blocks, and copy buttons.

**Files:**
- Modify: `lib/panko_web/components/blocks/user_prompt.ex`
- Modify: `lib/panko_web/components/blocks/tool_call.ex`
- Modify: `lib/panko_web/components/blocks/thinking.ex`
- Modify: `lib/panko_web/components/blocks/file_edit.ex`
- Modify: `lib/panko_web/components/blocks/sub_agent_spawn.ex`

**Step 1: Update UserPrompt**

```elixir
defmodule PankoWeb.Components.Blocks.UserPrompt do
  use Phoenix.Component

  def render(assigns) do
    ~H"""
    <article class="block border border-base-300 rounded-lg mb-4 border-l-4 border-l-info overflow-hidden">
      <div class="flex items-center justify-between px-4 py-2 bg-base-200/50 border-b border-base-300">
        <span class="text-sm font-semibold text-info">User</span>
        <time :if={@block.timestamp} class="text-xs text-base-content/50">
          {format_time(@block.timestamp)}
        </time>
      </div>
      <div class="px-4 py-3 whitespace-pre-wrap">
        {@block.content}
      </div>
    </article>
    """
  end

  defp format_time(nil), do: ""
  defp format_time(%DateTime{} = dt), do: Calendar.strftime(dt, "%H:%M:%S")
end
```

**Step 2: Update ToolCall with copy buttons and large output collapsing**

```elixir
defmodule PankoWeb.Components.Blocks.ToolCall do
  use Phoenix.Component

  @important_tools ~w(Write Edit Bash Read NotebookEdit)

  def render(assigns) do
    tool_name = assigns.block.metadata["name"] || "Tool"
    input = encode_metadata(assigns.block.metadata["input"])
    output = encode_metadata(assigns.block.metadata["output"])
    output_lines = if output != "", do: length(String.split(output, "\n")), else: 0
    is_important = tool_name in @important_tools
    is_large = output_lines > 100

    assigns =
      assigns
      |> assign(:tool_name, tool_name)
      |> assign(:encoded_input, input)
      |> assign(:encoded_output, output)
      |> assign(:output_lines, output_lines)
      |> assign(:is_important, is_important)
      |> assign(:is_large, is_large)

    ~H"""
    <article class="block border border-base-300 rounded-lg mb-4 border-l-4 border-l-warning overflow-hidden">
      <div class="flex items-center justify-between px-4 py-2 bg-base-200/50 border-b border-base-300">
        <span class="text-sm font-semibold text-warning">
          Tool: <span class="font-mono">{@tool_name}</span>
        </span>
        <time :if={@block.timestamp} class="text-xs text-base-content/50">
          {format_time(@block.timestamp)}
        </time>
      </div>
      <details class="group" open={@is_important}>
        <summary class="flex items-center justify-between px-4 py-2 cursor-pointer bg-base-200/30 border-b border-base-300 text-sm text-base-content/70 hover:bg-base-200/60">
          <span>Input</span>
          <button
            type="button"
            phx-click={copy_to_clipboard("tool-input-#{@block.id}")}
            class="btn btn-ghost btn-xs"
            title="Copy input"
          >
            <.icon name="hero-clipboard-micro" class="size-3" /> Copy
          </button>
        </summary>
        <pre id={"tool-input-#{@block.id}"} class="text-xs overflow-x-auto p-4 bg-base-300/30"><code>{@encoded_input}</code></pre>
      </details>
      <details :if={@encoded_output != ""} class="group" open={@is_important && !@is_large}>
        <summary class="flex items-center justify-between px-4 py-2 cursor-pointer bg-base-200/30 border-b border-base-300 text-sm text-base-content/70 hover:bg-base-200/60">
          <span>
            Output
            <span :if={@is_large} class="text-xs text-base-content/40 ml-1">({@output_lines} lines)</span>
          </span>
          <button
            type="button"
            phx-click={copy_to_clipboard("tool-output-#{@block.id}")}
            class="btn btn-ghost btn-xs"
            title="Copy output"
          >
            <.icon name="hero-clipboard-micro" class="size-3" /> Copy
          </button>
        </summary>
        <div class={["relative", @is_large && "max-h-96 overflow-hidden"]} id={"tool-output-wrap-#{@block.id}"}>
          <pre id={"tool-output-#{@block.id}"} class="text-xs overflow-x-auto p-4 bg-base-300/30"><code>{@encoded_output}</code></pre>
          <div :if={@is_large} class="absolute bottom-0 left-0 right-0">
            <div class="h-20 bg-gradient-to-t from-base-100 to-transparent" />
            <button
              type="button"
              phx-click={show_full_output("tool-output-wrap-#{@block.id}")}
              class="w-full py-2 text-sm text-warning bg-base-200 border-t border-base-300 hover:bg-base-300 cursor-pointer"
            >
              Show full output
            </button>
          </div>
        </div>
      </details>
    </article>
    """
  end

  defp copy_to_clipboard(target_id) do
    Phoenix.LiveView.JS.dispatch("phx:copy", to: "##{target_id}")
  end

  defp show_full_output(wrapper_id) do
    Phoenix.LiveView.JS.remove_class("max-h-96 overflow-hidden", to: "##{wrapper_id}")
    |> Phoenix.LiveView.JS.hide(to: "##{wrapper_id} > div:last-child")
  end

  defp format_time(nil), do: ""
  defp format_time(%DateTime{} = dt), do: Calendar.strftime(dt, "%H:%M:%S")

  defp encode_metadata(nil), do: ""
  defp encode_metadata(data) when is_map(data), do: Jason.encode!(data, pretty: true)
  defp encode_metadata(data) when is_binary(data), do: data
  defp encode_metadata(data), do: inspect(data)

  # Import icon component
  defp icon(assigns) do
    PankoWeb.CoreComponents.icon(assigns)
  end
end
```

**Step 3: Update Thinking**

```elixir
defmodule PankoWeb.Components.Blocks.Thinking do
  use Phoenix.Component

  def render(assigns) do
    ~H"""
    <article class="block border border-base-300 rounded-lg mb-4 border-l-4 border-l-secondary opacity-85 overflow-hidden">
      <details class="group">
        <summary class="flex items-center justify-between px-4 py-2 cursor-pointer bg-base-200/30 hover:bg-base-200/60">
          <span class="text-sm font-semibold text-secondary italic">Thinking</span>
          <time :if={@block.timestamp} class="text-xs text-base-content/50">
            {format_time(@block.timestamp)}
          </time>
        </summary>
        <div class="px-4 py-3 whitespace-pre-wrap text-sm italic text-base-content/70">
          {@block.content}
        </div>
      </details>
    </article>
    """
  end

  defp format_time(nil), do: ""
  defp format_time(%DateTime{} = dt), do: Calendar.strftime(dt, "%H:%M:%S")
end
```

**Step 4: Update FileEdit**

```elixir
defmodule PankoWeb.Components.Blocks.FileEdit do
  use Phoenix.Component

  def render(assigns) do
    file_path = get_file_path(assigns.block.metadata)
    tool_name = tool_name(assigns.block.metadata)

    assigns =
      assigns
      |> assign(:file_path, file_path)
      |> assign(:tool_name, tool_name)
      |> assign(:formatted_input, format_input(assigns.block.metadata))

    ~H"""
    <article class="block border border-base-300 rounded-lg mb-4 border-l-4 border-l-error overflow-hidden">
      <div class="flex items-center justify-between px-4 py-2 bg-base-200/50 border-b border-base-300">
        <span class="text-sm font-semibold text-error">
          <span class="badge badge-error badge-sm mr-2">{@tool_name}</span>
          <span class="font-mono text-xs">{@file_path}</span>
        </span>
        <time :if={@block.timestamp} class="text-xs text-base-content/50">
          {format_time(@block.timestamp)}
        </time>
      </div>
      <details open>
        <summary class="px-4 py-2 cursor-pointer bg-base-200/30 text-sm text-base-content/70 hover:bg-base-200/60">
          Content
        </summary>
        <pre class="text-xs overflow-x-auto p-4 bg-base-300/30"><code>{@formatted_input}</code></pre>
      </details>
    </article>
    """
  end

  defp get_file_path(nil), do: "unknown"
  defp get_file_path(%{"path" => path}) when is_binary(path), do: path
  defp get_file_path(%{"input" => %{"file_path" => path}}) when is_binary(path), do: path
  defp get_file_path(_), do: "unknown"

  defp tool_name(nil), do: "File Edit"
  defp tool_name(%{"name" => name}), do: name
  defp tool_name(_), do: "File Edit"

  defp format_input(nil), do: ""
  defp format_input(%{"input" => input}) when is_map(input), do: Jason.encode!(input, pretty: true)
  defp format_input(metadata) when is_map(metadata), do: Jason.encode!(metadata, pretty: true)
  defp format_input(_), do: ""

  defp format_time(nil), do: ""
  defp format_time(%DateTime{} = dt), do: Calendar.strftime(dt, "%H:%M:%S")
end
```

**Step 5: Update SubAgentSpawn**

```elixir
defmodule PankoWeb.Components.Blocks.SubAgentSpawn do
  use Phoenix.Component

  def render(assigns) do
    assigns =
      assigns
      |> assign(:agent_type, get_in(assigns.block.metadata, ["agent_type"]) || "unknown")
      |> assign(:description, get_in(assigns.block.metadata, ["description"]) || "")

    ~H"""
    <article class="block border border-base-300 rounded-lg mb-4 border-l-4 border-l-accent ml-8 overflow-hidden">
      <div class="flex items-center justify-between px-4 py-2 bg-base-200/50 border-b border-base-300">
        <span class="text-sm font-semibold text-accent">
          <span class={["badge badge-sm mr-2", agent_badge_class(@agent_type)]}>{@agent_type}</span>
          Sub-Agent
        </span>
        <time :if={@block.timestamp} class="text-xs text-base-content/50">
          {format_time(@block.timestamp)}
        </time>
      </div>
      <div class="px-4 py-3 text-sm">
        {@description}
      </div>
    </article>
    """
  end

  defp agent_badge_class("Explore"), do: "badge-info"
  defp agent_badge_class("Plan"), do: "badge-secondary"
  defp agent_badge_class("general-purpose"), do: "badge-success"
  defp agent_badge_class(_), do: "badge-neutral"

  defp format_time(nil), do: ""
  defp format_time(%DateTime{} = dt), do: Calendar.strftime(dt, "%H:%M:%S")
end
```

**Step 6: Verify compilation and tests**

Run: `mix compile --warnings-as-errors && mix test`
Expected: Clean compile, all tests pass

**Step 7: Commit**

```bash
git add lib/panko_web/components/blocks/
git commit -m "feat: update all block components with timestamps, copy buttons, article layout"
```

---

### Task 5: Build Project Accordion Sessions Page

Replace the flat session list with a project-grouped accordion that has a global search bar.

**Files:**
- Modify: `lib/panko_web/live/sessions_live.ex`
- Modify: `lib/panko/sessions.ex` (add domain function for project listing)
- Modify: `lib/panko/sessions/session.ex` (add project list action)

**Step 1: Add list_projects action to Session resource**

In `lib/panko/sessions/session.ex`, add inside the `actions` block:

```elixir
read :list_projects do
  prepare build(sort: [started_at: :desc])
end
```

**Step 2: Add domain function to Sessions**

In `lib/panko/sessions.ex`, add inside the `resource Panko.Sessions.Session` block:

```elixir
define :list_all_sessions, action: :list_projects
```

**Step 3: Rewrite SessionsLive**

Replace `lib/panko_web/live/sessions_live.ex`:

```elixir
defmodule PankoWeb.SessionsLive do
  use PankoWeb, :live_view

  require Ash.Query

  alias Panko.Sessions.Session

  @impl true
  def mount(_params, _session, socket) do
    if connected?(socket) do
      PankoWeb.Endpoint.subscribe("sessions:imported")
    end

    sessions = load_sessions()
    shared_session_ids = load_shared_session_ids()
    projects = group_by_project(sessions)

    {:ok,
     assign(socket,
       sessions: sessions,
       shared_session_ids: shared_session_ids,
       projects: projects,
       search_query: "",
       expanded_projects: MapSet.new(),
       page_title: "Sessions"
     )}
  end

  @impl true
  def handle_info(%Phoenix.Socket.Broadcast{topic: "sessions:imported"}, socket) do
    sessions = load_sessions()
    projects = group_by_project(sessions)

    projects =
      if socket.assigns.search_query != "" do
        filter_projects(projects, socket.assigns.search_query)
      else
        projects
      end

    {:noreply, assign(socket, sessions: sessions, projects: projects)}
  end

  @impl true
  def handle_event("search", %{"query" => query}, socket) do
    projects = group_by_project(socket.assigns.sessions)

    {filtered, expanded} =
      if query == "" do
        {projects, MapSet.new()}
      else
        filtered = filter_projects(projects, query)
        expanded = filtered |> Enum.map(fn {project, _} -> project end) |> MapSet.new()
        {filtered, expanded}
      end

    {:noreply, assign(socket, search_query: query, projects: filtered, expanded_projects: expanded)}
  end

  def handle_event("toggle_project", %{"project" => project}, socket) do
    expanded =
      if MapSet.member?(socket.assigns.expanded_projects, project) do
        MapSet.delete(socket.assigns.expanded_projects, project)
      else
        MapSet.put(socket.assigns.expanded_projects, project)
      end

    {:noreply, assign(socket, expanded_projects: expanded)}
  end

  @impl true
  def render(assigns) do
    ~H"""
    <div class="container mx-auto px-4 py-8">
      <div class="flex items-center justify-between mb-6">
        <h1 class="text-3xl font-bold">Sessions</h1>
        <div class="text-sm text-base-content/50">
          {length(@sessions)} sessions across {map_size(group_by_project(@sessions))} projects
        </div>
      </div>

      <%!-- Search bar --%>
      <div class="mb-6">
        <div class="relative">
          <.icon name="hero-magnifying-glass-micro" class="size-5 absolute left-3 top-1/2 -translate-y-1/2 text-base-content/40" />
          <input
            type="text"
            placeholder="Search sessions across all projects..."
            value={@search_query}
            phx-keyup="search"
            phx-key="*"
            phx-debounce="200"
            name="query"
            id="session-search"
            class="input input-bordered w-full pl-10"
            autocomplete="off"
          />
        </div>
      </div>

      <%!-- Empty state --%>
      <div :if={@projects == %{} && @search_query == ""} class="text-center py-12 text-base-content/60">
        <.icon name="hero-folder-open" class="size-12 mx-auto mb-4 opacity-40" />
        <p class="text-lg">No sessions found.</p>
        <p class="text-sm mt-2">Sessions from ~/.claude/projects/ will appear here automatically.</p>
      </div>

      <div :if={@projects == %{} && @search_query != ""} class="text-center py-12 text-base-content/60">
        <p class="text-lg">No results for "{@search_query}"</p>
      </div>

      <%!-- Project accordion --%>
      <div class="space-y-2">
        <div
          :for={{project, project_sessions} <- Enum.sort_by(@projects, fn {_p, sessions} -> sessions |> Enum.map(& &1.started_at) |> Enum.max(DateTime) end, {:desc, DateTime})}
          class="border border-base-300 rounded-lg overflow-hidden"
        >
          <%!-- Project header (accordion trigger) --%>
          <button
            phx-click="toggle_project"
            phx-value-project={project}
            class="w-full flex items-center justify-between px-4 py-3 bg-base-200/50 hover:bg-base-200 transition-colors cursor-pointer"
          >
            <div class="flex items-center gap-3">
              <.icon
                name={if MapSet.member?(@expanded_projects, project), do: "hero-chevron-down-micro", else: "hero-chevron-right-micro"}
                class="size-4 text-base-content/50"
              />
              <span class="font-semibold text-sm truncate">{display_project(project)}</span>
            </div>
            <div class="flex items-center gap-3 text-xs text-base-content/50">
              <span>{length(project_sessions)} sessions</span>
              <span>{format_relative_time(latest_activity(project_sessions))}</span>
            </div>
          </button>

          <%!-- Sessions list (collapsed by default) --%>
          <div :if={MapSet.member?(@expanded_projects, project)} class="border-t border-base-300">
            <.link
              :for={session <- project_sessions}
              navigate={~p"/sessions/#{session.id}"}
              class="flex items-center justify-between px-4 py-3 pl-11 hover:bg-base-200/30 transition-colors border-b border-base-300 last:border-b-0"
            >
              <div class="flex-1 min-w-0">
                <div class="flex items-center gap-2">
                  <span class="text-sm truncate">{session.title || "Untitled session"}</span>
                  <span
                    :if={MapSet.member?(@shared_session_ids, session.id)}
                    class="badge badge-success badge-xs gap-1 shrink-0"
                  >
                    Shared
                  </span>
                </div>
              </div>
              <div class="flex gap-4 text-xs text-base-content/50 shrink-0 ml-4">
                <span>{session.message_count || 0} msgs</span>
                <span>{session.block_count || 0} blocks</span>
                <span>{format_relative_time(session.started_at)}</span>
              </div>
            </.link>
          </div>
        </div>
      </div>
    </div>
    """
  end

  defp load_sessions do
    Session
    |> Ash.Query.sort(started_at: :desc)
    |> Ash.Query.load([:block_count, :message_count, :tool_call_count])
    |> Ash.read!()
  end

  defp load_shared_session_ids do
    Panko.Sharing.Share
    |> Ash.Query.filter(is_shared == true)
    |> Ash.Query.select([:session_id])
    |> Ash.read!()
    |> Enum.map(& &1.session_id)
    |> MapSet.new()
  end

  defp group_by_project(sessions) do
    Enum.group_by(sessions, fn s -> s.project || "Unknown Project" end)
  end

  defp filter_projects(projects, query) do
    query_down = String.downcase(query)

    projects
    |> Enum.map(fn {project, sessions} ->
      project_matches = String.contains?(String.downcase(project), query_down)

      matching_sessions =
        if project_matches do
          sessions
        else
          Enum.filter(sessions, fn s ->
            title = s.title || ""
            String.contains?(String.downcase(title), query_down)
          end)
        end

      {project, matching_sessions}
    end)
    |> Enum.reject(fn {_, sessions} -> sessions == [] end)
    |> Map.new()
  end

  defp display_project(project) do
    project
    |> String.replace(~r"^/home/[^/]+/", "~/")
    |> String.replace(~r"^/Users/[^/]+/", "~/")
  end

  defp latest_activity(sessions) do
    sessions
    |> Enum.map(& &1.started_at)
    |> Enum.max(DateTime, fn -> nil end)
  end

  defp format_relative_time(nil), do: ""

  defp format_relative_time(%DateTime{} = dt) do
    diff = DateTime.diff(DateTime.utc_now(), dt, :second)

    cond do
      diff < 60 -> "just now"
      diff < 3600 -> "#{div(diff, 60)}m ago"
      diff < 86400 -> "#{div(diff, 3600)}h ago"
      diff < 604_800 -> "#{div(diff, 86400)}d ago"
      true -> Calendar.strftime(dt, "%Y-%m-%d")
    end
  end
end
```

**Step 4: Verify compilation and tests**

Run: `mix compile --warnings-as-errors && mix test`
Expected: Clean compile, all tests pass

**Step 5: Format**

Run: `mix format`

**Step 6: Commit**

```bash
git add lib/panko_web/live/sessions_live.ex lib/panko/sessions.ex lib/panko/sessions/session.ex
git commit -m "feat: project accordion index page with global search"
```

---

### Task 6: Update SessionLive Detail Page

Update the session detail page with the article-based block layout and back link improvements.

**Files:**
- Modify: `lib/panko_web/live/session_live.ex`

**Step 1: Update SessionLive**

The main change: use the new article-based block layout, add footer with block count, display project as shortened path.

```elixir
defmodule PankoWeb.SessionLive do
  use PankoWeb, :live_view

  import PankoWeb.Components.Blocks

  alias PankoWeb.Components.ShareModal

  @impl true
  def mount(%{"id" => id}, _session, socket) do
    case Panko.Sessions.get_session(id) do
      {:ok, session} ->
        session = Ash.load!(session, [:blocks, :sub_agents, :block_count, :message_count])
        uri = get_connect_info_uri(socket)

        {:ok,
         assign(socket,
           session: session,
           page_title: session.title || "Session",
           uri: uri
         )}

      {:error, _} ->
        {:ok, push_navigate(socket, to: ~p"/")}
    end
  end

  @impl true
  def render(assigns) do
    ~H"""
    <div class="container mx-auto px-4 py-8 max-w-4xl">
      <div class="mb-6">
        <div class="flex items-center justify-between mb-4">
          <.link navigate={~p"/"} class="btn btn-ghost btn-sm gap-1">
            <.icon name="hero-arrow-left-micro" class="size-4" /> Sessions
          </.link>
          <.live_component
            module={ShareModal}
            id={"share-#{@session.id}"}
            session_id={@session.id}
            uri={@uri}
          />
        </div>
        <h1 class="text-2xl font-bold">{@session.title || "Untitled session"}</h1>
        <p class="text-sm text-base-content/60 mt-1 font-mono">{display_project(@session.project)}</p>
        <div class="flex gap-4 text-xs text-base-content/50 mt-2">
          <span>{@session.message_count} messages</span>
          <span>{@session.block_count} blocks</span>
          <span>{format_time(@session.started_at)}</span>
        </div>
      </div>

      <div class="space-y-2">
        <.block :for={blk <- @session.blocks} block={blk} />
      </div>

      <footer class="text-center text-xs text-base-content/40 mt-12 py-4 border-t border-base-300">
        {@session.block_count} blocks
      </footer>
    </div>
    """
  end

  defp display_project(nil), do: ""

  defp display_project(project) do
    project
    |> String.replace(~r"^/home/[^/]+/", "~/")
    |> String.replace(~r"^/Users/[^/]+/", "~/")
  end

  defp get_connect_info_uri(socket) do
    case get_connect_info(socket, :uri) do
      %URI{} = uri -> uri
      _ -> %URI{scheme: "http", host: "localhost", port: 4000}
    end
  end

  defp format_time(nil), do: ""
  defp format_time(%DateTime{} = dt), do: Calendar.strftime(dt, "%Y-%m-%d %H:%M")
end
```

**Step 2: Verify**

Run: `mix compile --warnings-as-errors && mix test`

**Step 3: Commit**

```bash
git add lib/panko_web/live/session_live.ex
git commit -m "feat: update session detail page layout"
```

---

### Task 7: Update ShareLive and phx:copy Handler

Update the shared session view to use the same article-based blocks and fix the copy event handler to work with `<pre>` elements.

**Files:**
- Modify: `lib/panko_web/live/share_live.ex`
- Modify: `assets/js/app.js`

**Step 1: Update ShareLive**

The share view needs the same block rendering improvements. Update the success render clause:

In the `render(assigns)` clause (the one without error), update to add the project path display and footer:

```elixir
def render(assigns) do
  ~H"""
  <div class="container mx-auto px-4 py-8 max-w-4xl">
    <h1 class="text-2xl font-bold mb-1">{@session.title || "Shared Session"}</h1>
    <p class="text-sm text-base-content/60 mb-6 font-mono">{display_project(@session.project)}</p>

    <div class="space-y-2">
      <.block :for={blk <- @session.blocks} block={blk} />
    </div>

    <footer class="text-center text-xs text-base-content/40 mt-12 py-4 border-t border-base-300">
      Shared with <a href="https://github.com/jordangarrison/panko" class="link">Panko</a>
    </footer>
  </div>
  """
end

defp display_project(nil), do: ""

defp display_project(project) do
  project
  |> String.replace(~r"^/home/[^/]+/", "~/")
  |> String.replace(~r"^/Users/[^/]+/", "~/")
end
```

**Step 2: Update phx:copy handler in app.js**

The copy handler needs to work with `<pre>` elements (for tool call copy buttons), not just inputs:

```javascript
// Handle phx:copy events for copying text to clipboard
window.addEventListener("phx:copy", (e) => {
  const text = e.target.value || e.target.textContent
  navigator.clipboard.writeText(text)
})
```

This already works since it falls back to `textContent`. No change needed if it already does this (verify first).

**Step 3: Verify**

Run: `mix compile --warnings-as-errors && mix test`

**Step 4: Commit**

```bash
git add lib/panko_web/live/share_live.ex assets/js/app.js
git commit -m "feat: update share view with improved block rendering"
```

---

### Task 8: Final Verification

**Step 1: Full test suite**

Run: `mix test`
Expected: All tests pass

**Step 2: Compile check**

Run: `mix compile --warnings-as-errors`
Expected: Clean

**Step 3: Format check**

Run: `mix format --check-formatted`
Expected: Clean

**Step 4: Manual verification**

Start the dev server and verify:
- Landing page shows project accordion
- Search filters across all projects/sessions
- Expanding a project shows its sessions
- Session detail page shows article-based blocks
- Assistant responses render markdown and handle command tags
- Tool calls have copy buttons and large output collapsing
- All block types show timestamps
- Share view works correctly

Run: `mix phx.server`

**Step 5: Commit any remaining fixes**

```bash
git add -A
git commit -m "fix: final adjustments from manual testing"
```

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

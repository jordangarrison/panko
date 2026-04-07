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
            <span :if={@is_large} class="text-xs text-base-content/40 ml-1">
              ({@output_lines} lines)
            </span>
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
        <div
          class={["relative", @is_large && "max-h-96 overflow-hidden"]}
          id={"tool-output-wrap-#{@block.id}"}
        >
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

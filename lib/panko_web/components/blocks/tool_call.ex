defmodule PankoWeb.Components.Blocks.ToolCall do
  use Phoenix.Component

  def render(assigns) do
    assigns = assign(assigns, :encoded_input, encode_metadata(assigns.block.metadata["input"]))
    assigns = assign(assigns, :encoded_output, encode_metadata(assigns.block.metadata["output"]))

    ~H"""
    <div class="collapse collapse-arrow bg-base-200 mb-4">
      <input type="checkbox" />
      <div class="collapse-title font-mono text-sm">
        <span class="badge badge-outline badge-sm mr-2">{@block.metadata["name"]}</span> Tool Call
      </div>
      <div class="collapse-content">
        <pre class="text-xs overflow-x-auto"><code>{@encoded_input}</code></pre>
        <div :if={@block.metadata["output"]} class="mt-2 border-t border-base-300 pt-2">
          <p class="text-xs font-semibold mb-1">Output:</p>
          <pre class="text-xs overflow-x-auto"><code>{@encoded_output}</code></pre>
        </div>
      </div>
    </div>
    """
  end

  defp encode_metadata(nil), do: ""
  defp encode_metadata(data) when is_map(data), do: Jason.encode!(data, pretty: true)
  defp encode_metadata(data) when is_binary(data), do: data
  defp encode_metadata(data), do: inspect(data)
end

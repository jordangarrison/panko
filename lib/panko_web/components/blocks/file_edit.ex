defmodule PankoWeb.Components.Blocks.FileEdit do
  use Phoenix.Component

  def render(assigns) do
    assigns = assign(assigns, :file_path, get_file_path(assigns.block.metadata))

    ~H"""
    <div class="collapse collapse-arrow bg-base-200 mb-4">
      <input type="checkbox" />
      <div class="collapse-title font-mono text-sm">
        <span class="badge badge-info badge-sm mr-2">{tool_name(@block.metadata)}</span>
        <span class="truncate">{@file_path}</span>
      </div>
      <div class="collapse-content">
        <pre class="text-xs overflow-x-auto"><code>{format_input(@block.metadata)}</code></pre>
      </div>
    </div>
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

  defp format_input(%{"input" => input}) when is_map(input),
    do: Jason.encode!(input, pretty: true)

  defp format_input(metadata) when is_map(metadata), do: Jason.encode!(metadata, pretty: true)
  defp format_input(_), do: ""
end

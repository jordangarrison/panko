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

  defp format_input(%{"input" => input}) when is_map(input),
    do: Jason.encode!(input, pretty: true)

  defp format_input(metadata) when is_map(metadata), do: Jason.encode!(metadata, pretty: true)
  defp format_input(_), do: ""

  defp format_time(nil), do: ""
  defp format_time(%DateTime{} = dt), do: Calendar.strftime(dt, "%H:%M:%S")
end

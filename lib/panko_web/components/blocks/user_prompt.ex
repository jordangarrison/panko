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

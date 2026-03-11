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

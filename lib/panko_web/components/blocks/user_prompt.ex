defmodule PankoWeb.Components.Blocks.UserPrompt do
  use Phoenix.Component

  def render(assigns) do
    ~H"""
    <div class="chat chat-end mb-4">
      <div class="chat-bubble chat-bubble-primary whitespace-pre-wrap">
        {@block.content}
      </div>
      <div class="chat-footer text-xs opacity-50">
        {format_time(@block.timestamp)}
      </div>
    </div>
    """
  end

  defp format_time(nil), do: ""
  defp format_time(%DateTime{} = dt), do: Calendar.strftime(dt, "%H:%M:%S")
end

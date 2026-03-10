defmodule PankoWeb.Components.Blocks.AssistantResponse do
  use Phoenix.Component

  def render(assigns) do
    ~H"""
    <div class="chat chat-start mb-4">
      <div class="chat-bubble whitespace-pre-wrap prose prose-sm max-w-none">
        {@block.content}
      </div>
    </div>
    """
  end
end

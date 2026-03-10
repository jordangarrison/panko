defmodule PankoWeb.Components.Blocks.Thinking do
  use Phoenix.Component

  def render(assigns) do
    ~H"""
    <div class="collapse collapse-arrow bg-base-200/50 mb-4">
      <input type="checkbox" />
      <div class="collapse-title text-sm italic opacity-60">
        Thinking...
      </div>
      <div class="collapse-content">
        <p class="whitespace-pre-wrap text-sm italic opacity-70">
          {@block.content}
        </p>
      </div>
    </div>
    """
  end
end

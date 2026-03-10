defmodule PankoWeb.Components.Blocks.SubAgentSpawn do
  use Phoenix.Component

  def render(assigns) do
    assigns =
      assigns
      |> assign(:agent_type, get_in(assigns.block.metadata, ["agent_type"]) || "unknown")
      |> assign(:description, get_in(assigns.block.metadata, ["description"]) || "")

    ~H"""
    <div class="alert mb-4">
      <div>
        <span class="badge badge-accent badge-sm mr-2">Agent</span>
        <span class="badge badge-outline badge-sm mr-2">{@agent_type}</span>
        <span class="text-sm">{@description}</span>
      </div>
    </div>
    """
  end
end

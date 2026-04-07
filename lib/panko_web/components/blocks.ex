defmodule PankoWeb.Components.Blocks do
  use Phoenix.Component

  alias PankoWeb.Components.Blocks.{
    UserPrompt,
    AssistantResponse,
    ToolCall,
    Thinking,
    FileEdit,
    SubAgentSpawn
  }

  attr :block, :map, required: true

  def block(%{block: %{block_type: :user_prompt}} = assigns), do: UserPrompt.render(assigns)

  def block(%{block: %{block_type: :assistant_response}} = assigns),
    do: AssistantResponse.render(assigns)

  def block(%{block: %{block_type: :tool_call}} = assigns), do: ToolCall.render(assigns)
  def block(%{block: %{block_type: :thinking}} = assigns), do: Thinking.render(assigns)
  def block(%{block: %{block_type: :file_edit}} = assigns), do: FileEdit.render(assigns)

  def block(%{block: %{block_type: :sub_agent_spawn}} = assigns),
    do: SubAgentSpawn.render(assigns)

  def block(assigns), do: ~H""
end

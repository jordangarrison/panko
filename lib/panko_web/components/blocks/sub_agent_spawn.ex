defmodule PankoWeb.Components.Blocks.SubAgentSpawn do
  use Phoenix.Component

  def render(assigns) do
    assigns =
      assigns
      |> assign(:agent_type, get_in(assigns.block.metadata, ["agent_type"]) || "unknown")
      |> assign(:description, get_in(assigns.block.metadata, ["description"]) || "")

    ~H"""
    <article class="block border border-base-300 rounded-lg mb-4 border-l-4 border-l-accent ml-8 overflow-hidden">
      <div class="flex items-center justify-between px-4 py-2 bg-base-200/50 border-b border-base-300">
        <span class="text-sm font-semibold text-accent">
          <span class={["badge badge-sm mr-2", agent_badge_class(@agent_type)]}>{@agent_type}</span>
          Sub-Agent
        </span>
        <time :if={@block.timestamp} class="text-xs text-base-content/50">
          {format_time(@block.timestamp)}
        </time>
      </div>
      <div class="px-4 py-3 text-sm">
        {@description}
      </div>
    </article>
    """
  end

  defp agent_badge_class("Explore"), do: "badge-info"
  defp agent_badge_class("Plan"), do: "badge-secondary"
  defp agent_badge_class("general-purpose"), do: "badge-success"
  defp agent_badge_class(_), do: "badge-neutral"

  defp format_time(nil), do: ""
  defp format_time(%DateTime{} = dt), do: Calendar.strftime(dt, "%H:%M:%S")
end

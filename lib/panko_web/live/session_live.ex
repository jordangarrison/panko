defmodule PankoWeb.SessionLive do
  use PankoWeb, :live_view

  import PankoWeb.Components.Blocks

  @impl true
  def mount(%{"id" => id}, _session, socket) do
    case Panko.Sessions.get_session(id) do
      {:ok, session} ->
        session = Ash.load!(session, [:blocks, :sub_agents, :block_count, :message_count])
        {:ok, assign(socket, session: session, page_title: session.title || "Session")}

      {:error, _} ->
        {:ok, push_navigate(socket, to: ~p"/")}
    end
  end

  @impl true
  def render(assigns) do
    ~H"""
    <div class="container mx-auto px-4 py-8 max-w-4xl">
      <div class="mb-6">
        <.link navigate={~p"/"} class="btn btn-ghost btn-sm mb-4">&larr; Back</.link>
        <h1 class="text-2xl font-bold">{@session.title || "Untitled session"}</h1>
        <p class="text-sm text-base-content/60 mt-1">{@session.project}</p>
        <div class="flex gap-4 text-xs text-base-content/50 mt-2">
          <span>{@session.message_count} messages</span>
          <span>{@session.block_count} blocks</span>
          <span>{format_time(@session.started_at)}</span>
        </div>
      </div>

      <div class="space-y-2">
        <.block :for={blk <- @session.blocks} block={blk} />
      </div>
    </div>
    """
  end

  defp format_time(nil), do: ""
  defp format_time(%DateTime{} = dt), do: Calendar.strftime(dt, "%Y-%m-%d %H:%M")
end

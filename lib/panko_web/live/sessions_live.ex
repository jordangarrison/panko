defmodule PankoWeb.SessionsLive do
  use PankoWeb, :live_view

  alias Panko.Sessions.Session

  @impl true
  def mount(_params, _session, socket) do
    if connected?(socket) do
      PankoWeb.Endpoint.subscribe("sessions:imported")
    end

    sessions = load_sessions()
    {:ok, assign(socket, sessions: sessions, page_title: "Sessions")}
  end

  @impl true
  def handle_info(%Phoenix.Socket.Broadcast{topic: "sessions:imported"}, socket) do
    sessions = load_sessions()
    {:noreply, assign(socket, sessions: sessions)}
  end

  @impl true
  def render(assigns) do
    ~H"""
    <div class="container mx-auto px-4 py-8">
      <h1 class="text-3xl font-bold mb-8">Sessions</h1>

      <div :if={@sessions == []} class="text-center py-12 text-base-content/60">
        <p class="text-lg">No sessions found.</p>
        <p class="text-sm mt-2">Sessions from ~/.claude/projects/ will appear here automatically.</p>
      </div>

      <div class="grid gap-4">
        <.link
          :for={session <- @sessions}
          navigate={~p"/sessions/#{session.id}"}
          class="card bg-base-200 shadow-sm hover:shadow-md transition-shadow"
        >
          <div class="card-body">
            <h2 class="card-title text-sm font-mono">
              {session.title || "Untitled session"}
            </h2>
            <p class="text-xs text-base-content/60">{session.project}</p>
            <div class="flex gap-4 text-xs text-base-content/50 mt-2">
              <span>{session.message_count || 0} messages</span>
              <span>{session.block_count || 0} blocks</span>
              <span>{format_time(session.started_at)}</span>
            </div>
          </div>
        </.link>
      </div>
    </div>
    """
  end

  defp load_sessions do
    Session
    |> Ash.Query.sort(started_at: :desc)
    |> Ash.Query.limit(50)
    |> Ash.Query.load([:block_count, :message_count, :tool_call_count])
    |> Ash.read!()
  end

  defp format_time(nil), do: ""

  defp format_time(%DateTime{} = dt) do
    Calendar.strftime(dt, "%Y-%m-%d %H:%M")
  end
end

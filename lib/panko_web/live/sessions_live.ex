defmodule PankoWeb.SessionsLive do
  use PankoWeb, :live_view

  require Ash.Query

  alias Panko.Sessions.Session

  @impl true
  def mount(_params, _session, socket) do
    if connected?(socket) do
      PankoWeb.Endpoint.subscribe("sessions:imported")
    end

    sessions = load_sessions()
    shared_session_ids = load_shared_session_ids()
    {:ok, assign(socket, sessions: sessions, shared_session_ids: shared_session_ids, page_title: "Sessions")}
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
            <div class="flex items-center justify-between">
              <h2 class="card-title text-sm font-mono">
                {session.title || "Untitled session"}
              </h2>
              <span
                :if={MapSet.member?(@shared_session_ids, session.id)}
                class="badge badge-success badge-sm gap-1"
                title="Shared"
              >
                <.icon name="hero-link-micro" class="size-3" /> Shared
              </span>
            </div>
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

  defp load_shared_session_ids do
    Panko.Sharing.Share
    |> Ash.Query.filter(is_shared == true)
    |> Ash.Query.select([:session_id])
    |> Ash.read!()
    |> Enum.map(& &1.session_id)
    |> MapSet.new()
  end

  defp format_time(nil), do: ""

  defp format_time(%DateTime{} = dt) do
    Calendar.strftime(dt, "%Y-%m-%d %H:%M")
  end
end

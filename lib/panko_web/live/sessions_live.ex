defmodule PankoWeb.SessionsLive do
  use PankoWeb, :live_view

  alias Panko.Sessions
  alias Panko.Sharing

  @impl true
  def mount(_params, _session, socket) do
    if connected?(socket) do
      PankoWeb.Endpoint.subscribe("sessions:imported")
    end

    sessions = load_sessions()
    shared_session_ids = load_shared_session_ids()
    projects = group_by_project(sessions)

    {:ok,
     assign(socket,
       sessions: sessions,
       shared_session_ids: shared_session_ids,
       projects: projects,
       project_count: map_size(projects),
       search_query: "",
       expanded_projects: MapSet.new(),
       page_title: "Sessions"
     )}
  end

  @impl true
  def handle_info(%Phoenix.Socket.Broadcast{topic: "sessions:imported"}, socket) do
    sessions = load_sessions()
    projects = group_by_project(sessions)

    projects =
      if socket.assigns.search_query != "" do
        filter_projects(projects, socket.assigns.search_query)
      else
        projects
      end

    {:noreply,
     assign(socket, sessions: sessions, projects: projects, project_count: map_size(projects))}
  end

  @impl true
  def handle_event("search", %{"query" => query}, socket) do
    projects = group_by_project(socket.assigns.sessions)

    {filtered, expanded} =
      if query == "" do
        {projects, MapSet.new()}
      else
        filtered = filter_projects(projects, query)
        expanded = filtered |> Enum.map(fn {project, _} -> project end) |> MapSet.new()
        {filtered, expanded}
      end

    {:noreply,
     assign(socket, search_query: query, projects: filtered, expanded_projects: expanded)}
  end

  @impl true
  def handle_event("toggle_project", %{"project" => project}, socket) do
    expanded =
      if MapSet.member?(socket.assigns.expanded_projects, project) do
        MapSet.delete(socket.assigns.expanded_projects, project)
      else
        MapSet.put(socket.assigns.expanded_projects, project)
      end

    {:noreply, assign(socket, expanded_projects: expanded)}
  end

  @impl true
  def render(assigns) do
    ~H"""
    <div class="container mx-auto px-4 py-8">
      <div class="flex items-center justify-between mb-6">
        <h1 class="text-3xl font-bold">Sessions</h1>
        <div class="text-sm text-base-content/50">
          {length(@sessions)} sessions across {@project_count} projects
        </div>
      </div>

      <%!-- Search bar --%>
      <div class="mb-6">
        <div class="relative">
          <.icon
            name="hero-magnifying-glass-micro"
            class="size-5 absolute left-3 top-1/2 -translate-y-1/2 text-base-content/40"
          />
          <input
            type="text"
            placeholder="Search sessions across all projects..."
            value={@search_query}
            phx-keyup="search"
            phx-key="*"
            phx-debounce="200"
            name="query"
            id="session-search"
            class="input input-bordered w-full pl-10"
            autocomplete="off"
          />
        </div>
      </div>

      <%!-- Empty state --%>
      <div
        :if={@projects == %{} && @search_query == ""}
        class="text-center py-12 text-base-content/60"
      >
        <.icon name="hero-folder-open" class="size-12 mx-auto mb-4 opacity-40" />
        <p class="text-lg">No sessions found.</p>
        <p class="text-sm mt-2">
          Sessions from ~/.claude/projects/ will appear here automatically.
        </p>
      </div>

      <div
        :if={@projects == %{} && @search_query != ""}
        class="text-center py-12 text-base-content/60"
      >
        <p class="text-lg">No results for "{@search_query}"</p>
      </div>

      <%!-- Project accordion --%>
      <div class="space-y-2">
        <div
          :for={
            {project, project_sessions} <-
              Enum.sort_by(
                @projects,
                fn {_p, sessions} ->
                  sessions |> Enum.map(& &1.started_at) |> Enum.max(DateTime)
                end,
                {:desc, DateTime}
              )
          }
          class="border border-base-300 rounded-lg overflow-hidden"
        >
          <%!-- Project header (accordion trigger) --%>
          <button
            phx-click="toggle_project"
            phx-value-project={project}
            class="w-full flex items-center justify-between px-4 py-3 bg-base-200/50 hover:bg-base-200 transition-colors cursor-pointer"
          >
            <div class="flex items-center gap-3">
              <.icon
                name={
                  if MapSet.member?(@expanded_projects, project),
                    do: "hero-chevron-down-micro",
                    else: "hero-chevron-right-micro"
                }
                class="size-4 text-base-content/50"
              />
              <span class="font-semibold text-sm truncate">{display_project(project)}</span>
            </div>
            <div class="flex items-center gap-3 text-xs text-base-content/50">
              <span>{length(project_sessions)} sessions</span>
              <span>{format_relative_time(latest_activity(project_sessions))}</span>
            </div>
          </button>

          <%!-- Sessions list (collapsed by default) --%>
          <div :if={MapSet.member?(@expanded_projects, project)} class="border-t border-base-300">
            <.link
              :for={session <- project_sessions}
              navigate={~p"/sessions/#{session.id}"}
              class="flex items-center justify-between px-4 py-3 pl-11 hover:bg-base-200/30 transition-colors border-b border-base-300 last:border-b-0"
            >
              <div class="flex-1 min-w-0">
                <div class="flex items-center gap-2">
                  <span class="text-sm truncate">{session.title || "Untitled session"}</span>
                  <span
                    :if={MapSet.member?(@shared_session_ids, session.id)}
                    class="badge badge-success badge-xs gap-1 shrink-0"
                  >
                    Shared
                  </span>
                </div>
              </div>
              <div class="flex gap-4 text-xs text-base-content/50 shrink-0 ml-4">
                <span>{session.message_count || 0} msgs</span>
                <span>{session.block_count || 0} blocks</span>
                <span>{format_relative_time(session.started_at)}</span>
              </div>
            </.link>
          </div>
        </div>
      </div>
    </div>
    """
  end

  defp load_sessions do
    Sessions.list_all_sessions!()
  end

  defp load_shared_session_ids do
    Sharing.list_shared_session_ids!()
    |> Enum.map(& &1.session_id)
    |> MapSet.new()
  end

  defp group_by_project(sessions) do
    Enum.group_by(sessions, fn s -> s.project || "Unknown Project" end)
  end

  defp filter_projects(projects, query) do
    query_down = String.downcase(query)

    projects
    |> Enum.map(fn {project, sessions} ->
      project_matches = String.contains?(String.downcase(project), query_down)

      matching_sessions =
        if project_matches do
          sessions
        else
          Enum.filter(sessions, fn s ->
            title = s.title || ""
            String.contains?(String.downcase(title), query_down)
          end)
        end

      {project, matching_sessions}
    end)
    |> Enum.reject(fn {_, sessions} -> sessions == [] end)
    |> Map.new()
  end

  defp display_project(project) do
    project
    |> String.replace(~r"^/home/[^/]+/", "~/")
    |> String.replace(~r"^/Users/[^/]+/", "~/")
  end

  defp latest_activity(sessions) do
    sessions
    |> Enum.map(& &1.started_at)
    |> Enum.max(DateTime, fn -> nil end)
  end

  defp format_relative_time(nil), do: ""

  defp format_relative_time(%DateTime{} = dt) do
    diff = DateTime.diff(DateTime.utc_now(), dt, :second)

    cond do
      diff < 60 -> "just now"
      diff < 3600 -> "#{div(diff, 60)}m ago"
      diff < 86400 -> "#{div(diff, 3600)}h ago"
      diff < 604_800 -> "#{div(diff, 86400)}d ago"
      true -> Calendar.strftime(dt, "%Y-%m-%d")
    end
  end
end

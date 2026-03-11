defmodule PankoWeb.SessionLive do
  use PankoWeb, :live_view

  import PankoWeb.Components.Blocks

  alias PankoWeb.Components.ShareModal

  @impl true
  def mount(%{"id" => id}, _session, socket) do
    case Panko.Sessions.get_session(id) do
      {:ok, session} ->
        session = Ash.load!(session, [:blocks, :sub_agents, :block_count, :message_count])
        uri = get_connect_info_uri(socket)

        {:ok,
         assign(socket,
           session: session,
           page_title: session.title || "Session",
           uri: uri
         )}

      {:error, _} ->
        {:ok, push_navigate(socket, to: ~p"/")}
    end
  end

  @impl true
  def render(assigns) do
    ~H"""
    <div class="container mx-auto px-4 py-8 max-w-4xl">
      <div class="mb-6">
        <div class="flex items-center justify-between mb-4">
          <.link navigate={~p"/"} class="btn btn-ghost btn-sm gap-1">
            <.icon name="hero-arrow-left-micro" class="size-4" /> Sessions
          </.link>
          <.live_component
            module={ShareModal}
            id={"share-#{@session.id}"}
            session_id={@session.id}
            uri={@uri}
          />
        </div>
        <h1 class="text-2xl font-bold">{@session.title || "Untitled session"}</h1>
        <p class="text-sm text-base-content/60 mt-1 font-mono">{display_project(@session.project)}</p>
        <div class="flex gap-4 text-xs text-base-content/50 mt-2">
          <span>{@session.message_count} messages</span>
          <span>{@session.block_count} blocks</span>
          <span>{format_time(@session.started_at)}</span>
        </div>
      </div>

      <div class="space-y-2">
        <.block :for={blk <- @session.blocks} block={blk} />
      </div>

      <footer class="text-center text-xs text-base-content/40 mt-12 py-4 border-t border-base-300">
        {@session.block_count} blocks
      </footer>
    </div>
    """
  end

  defp display_project(nil), do: ""

  defp display_project(project) do
    project
    |> String.replace(~r"^/home/[^/]+/", "~/")
    |> String.replace(~r"^/Users/[^/]+/", "~/")
  end

  defp get_connect_info_uri(socket) do
    case get_connect_info(socket, :uri) do
      %URI{} = uri -> uri
      _ -> %URI{scheme: "http", host: "localhost", port: 4000}
    end
  end

  defp format_time(nil), do: ""
  defp format_time(%DateTime{} = dt), do: Calendar.strftime(dt, "%Y-%m-%d %H:%M")
end

defmodule PankoWeb.ShareLive do
  use PankoWeb, :live_view

  import PankoWeb.Components.Blocks

  @impl true
  def mount(%{"slug" => slug}, _session, socket) do
    case Panko.Sharing.get_share_by_slug(slug) do
      {:ok, share} ->
        if expired?(share) do
          {:ok, assign(socket, :error, :expired)}
        else
          session = share.session

          {:ok,
           assign(socket,
             share: share,
             session: session,
             page_title: session.title || "Shared Session"
           )}
        end

      {:error, _} ->
        {:ok, assign(socket, :error, :not_found)}
    end
  end

  @impl true
  def render(%{error: :not_found} = assigns) do
    ~H"""
    <div class="flex items-center justify-center min-h-screen">
      <div class="text-center">
        <h1 class="text-4xl font-bold mb-4">404</h1>
        <p class="text-base-content/60">This share link is not available.</p>
      </div>
    </div>
    """
  end

  def render(%{error: :expired} = assigns) do
    ~H"""
    <div class="flex items-center justify-center min-h-screen">
      <div class="text-center">
        <h1 class="text-4xl font-bold mb-4">Expired</h1>
        <p class="text-base-content/60">This shared session has expired.</p>
      </div>
    </div>
    """
  end

  def render(assigns) do
    ~H"""
    <div class="container mx-auto px-4 py-8 max-w-4xl">
      <h1 class="text-2xl font-bold mb-1">{@session.title || "Shared Session"}</h1>
      <p class="text-sm text-base-content/60 mb-6 font-mono">{display_project(@session.project)}</p>

      <div class="space-y-2">
        <.block :for={blk <- @session.blocks} block={blk} />
      </div>

      <footer class="text-center text-xs text-base-content/40 mt-12 py-4 border-t border-base-300">
        Shared with <a href="https://github.com/jordangarrison/panko" class="link">Panko</a>
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

  defp expired?(%{expires_at: nil}), do: false

  defp expired?(%{expires_at: expires_at}) do
    DateTime.compare(DateTime.utc_now(), expires_at) == :gt
  end
end

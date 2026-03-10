defmodule PankoWeb.Components.ShareModal do
  @moduledoc """
  A LiveComponent that handles sharing a session.

  Shows a share button that creates a share, displays the URL with a copy button,
  and offers unpublish/republish controls.
  """
  use PankoWeb, :live_component

  require Ash.Query

  alias Panko.Sharing
  alias Panko.Sharing.Share

  @impl true
  def update(assigns, socket) do
    share = find_share(assigns.session_id)

    {:ok,
     socket
     |> assign(assigns)
     |> assign_new(:copied, fn -> false end)
     |> assign_new(:show_modal, fn -> false end)
     |> assign(share: share)}
  end

  @impl true
  def render(assigns) do
    ~H"""
    <div>
      <%!-- No share exists: show "Share" button --%>
      <button
        :if={is_nil(@share)}
        phx-click="create_share"
        phx-target={@myself}
        class="btn btn-primary btn-sm gap-2"
      >
        <.icon name="hero-share-micro" class="size-4" /> Share
      </button>

      <%!-- Share exists and is active: show "Shared" button that opens modal --%>
      <button
        :if={@share && @share.is_shared}
        phx-click={open_modal("share-modal-#{@id}")}
        class="btn btn-success btn-sm btn-soft gap-2"
      >
        <.icon name="hero-link-micro" class="size-4" /> Shared
      </button>

      <%!-- Share exists but unpublished: show "Reshare" button --%>
      <button
        :if={@share && !@share.is_shared}
        phx-click="republish_share"
        phx-target={@myself}
        class="btn btn-ghost btn-sm gap-2"
      >
        <.icon name="hero-share-micro" class="size-4" /> Reshare
      </button>

      <%!-- Modal dialog for active shares --%>
      <dialog
        :if={@share && @share.is_shared}
        id={"share-modal-#{@id}"}
        class="modal"
        phx-mounted={@show_modal && open_modal("share-modal-#{@id}")}
      >
        <div class="modal-box">
          <form method="dialog">
            <button class="btn btn-sm btn-circle btn-ghost absolute right-2 top-2">
              <.icon name="hero-x-mark" class="size-4" />
            </button>
          </form>

          <h3 class="text-lg font-bold mb-4">Session Shared</h3>

          <div class="flex items-center gap-2 mb-4">
            <input
              type="text"
              value={share_url(@share, @uri)}
              readonly
              class="input input-bordered w-full font-mono text-sm"
              id={"share-url-#{@id}"}
            />
            <button
              phx-click={
                JS.dispatch("phx:copy", to: "#share-url-#{@id}")
                |> JS.push("mark_copied", target: @myself)
              }
              class="btn btn-square btn-sm"
              title="Copy URL"
            >
              <.icon
                name={if @copied, do: "hero-check-micro", else: "hero-clipboard-micro"}
                class="size-4"
              />
            </button>
          </div>

          <p class="text-xs text-base-content/60 mb-4">
            Anyone with this link can view this session.
          </p>

          <div class="modal-action">
            <button
              phx-click="unpublish_share"
              phx-target={@myself}
              class="btn btn-error btn-sm btn-soft"
            >
              <.icon name="hero-eye-slash-micro" class="size-4" /> Unpublish
            </button>
            <form method="dialog">
              <button class="btn btn-sm">Close</button>
            </form>
          </div>
        </div>
        <form method="dialog" class="modal-backdrop">
          <button>close</button>
        </form>
      </dialog>
    </div>
    """
  end

  @impl true
  def handle_event("create_share", _params, socket) do
    case Sharing.create_share(socket.assigns.session_id) do
      {:ok, share} ->
        {:noreply, assign(socket, share: share, show_modal: true, copied: false)}

      {:error, _changeset} ->
        {:noreply, put_flash(socket, :error, "Failed to create share")}
    end
  end

  def handle_event("unpublish_share", _params, socket) do
    case Sharing.unpublish_share(socket.assigns.share) do
      {:ok, share} ->
        {:noreply, assign(socket, share: share, show_modal: false)}

      {:error, _changeset} ->
        {:noreply, put_flash(socket, :error, "Failed to unpublish share")}
    end
  end

  def handle_event("republish_share", _params, socket) do
    case Sharing.republish_share(socket.assigns.share) do
      {:ok, share} ->
        {:noreply, assign(socket, share: share, show_modal: true, copied: false)}

      {:error, _changeset} ->
        {:noreply, put_flash(socket, :error, "Failed to republish share")}
    end
  end

  def handle_event("mark_copied", _params, socket) do
    {:noreply, assign(socket, copied: true)}
  end

  defp find_share(session_id) do
    Share
    |> Ash.Query.filter(session_id: session_id)
    |> Ash.Query.sort(inserted_at: :desc)
    |> Ash.Query.limit(1)
    |> Ash.read!()
    |> List.first()
  end

  defp share_url(share, uri) do
    "#{uri.scheme}://#{uri.host}#{port_string(uri)}/s/#{share.slug}"
  end

  defp port_string(%URI{scheme: "https", port: 443}), do: ""
  defp port_string(%URI{scheme: "http", port: 80}), do: ""
  defp port_string(%URI{port: nil}), do: ""
  defp port_string(%URI{port: port}), do: ":#{port}"

  defp open_modal(id) do
    JS.dispatch("modal:open", to: "##{id}")
  end
end

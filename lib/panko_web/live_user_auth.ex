defmodule PankoWeb.LiveUserAuth do
  @moduledoc """
  LiveView on_mount hook that redirects unauthenticated users to sign-in.
  """
  import Phoenix.LiveView

  def on_mount(:live_user_required, _params, _session, socket) do
    if socket.assigns[:current_user] do
      {:cont, socket}
    else
      {:halt, redirect(socket, to: "/sign-in")}
    end
  end
end

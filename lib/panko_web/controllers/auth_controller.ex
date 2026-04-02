defmodule PankoWeb.AuthController do
  use PankoWeb, :controller
  use AshAuthentication.Phoenix.Controller

  def success(conn, _activity, user, _token) do
    conn
    |> store_in_session(user)
    |> assign(:current_user, user)
    |> redirect(to: ~p"/")
  end

  def failure(conn, _activity, _reason) do
    conn
    |> put_flash(:error, "Authentication failed")
    |> redirect(to: "/sign-in")
  end

  def sign_out(conn, _params) do
    conn
    |> clear_session(:panko)
    |> redirect(to: "/sign-in")
  end
end

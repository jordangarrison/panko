defmodule PankoWeb.Router do
  use PankoWeb, :router
  use AshAuthentication.Phoenix.Router

  pipeline :browser do
    plug :accepts, ["html"]
    plug :fetch_session
    plug :fetch_live_flash
    plug :put_root_layout, html: {PankoWeb.Layouts, :root}
    plug :protect_from_forgery
    plug :put_secure_browser_headers
    plug :load_from_session
  end

  pipeline :api do
    plug :accepts, ["json"]
  end

  # Public auth routes (sign in, register, sign out, auth callbacks)
  scope "/", PankoWeb do
    pipe_through :browser

    sign_in_route(register_path: "/register", auth_routes_prefix: "/auth")
    sign_out_route AuthController
    auth_routes AuthController, Panko.Accounts.User, path: "/auth"
  end

  # Protected routes -- require authenticated user
  scope "/", PankoWeb do
    pipe_through :browser

    ash_authentication_live_session :authenticated,
      otp_app: :panko,
      on_mount: [{PankoWeb.LiveUserAuth, :live_user_required}],
      layout: {PankoWeb.Layouts, :app} do
      live "/", SessionsLive, :index
      live "/sessions/:id", SessionLive, :show
    end
  end

  # Public share routes -- no auth
  scope "/s", PankoWeb do
    pipe_through :browser

    live_session :public do
      live "/:slug", ShareLive, :show
    end
  end
end

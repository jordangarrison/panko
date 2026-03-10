defmodule PankoWeb.Router do
  use PankoWeb, :router

  pipeline :browser do
    plug :accepts, ["html"]
    plug :fetch_session
    plug :fetch_live_flash
    plug :put_root_layout, html: {PankoWeb.Layouts, :root}
    plug :protect_from_forgery
    plug :put_secure_browser_headers
  end

  pipeline :api do
    plug :accepts, ["json"]
  end

  scope "/", PankoWeb do
    pipe_through :browser

    live_session :default, layout: {PankoWeb.Layouts, :app} do
      live "/", SessionsLive, :index
      live "/sessions/:id", SessionLive, :show
    end
  end

  scope "/s", PankoWeb do
    pipe_through :browser

    live_session :public do
      live "/:slug", ShareLive, :show
    end
  end

  # Other scopes may use custom stacks.
  # scope "/api", PankoWeb do
  #   pipe_through :api
  # end
end

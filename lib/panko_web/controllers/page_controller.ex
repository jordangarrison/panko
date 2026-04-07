defmodule PankoWeb.PageController do
  use PankoWeb, :controller

  def home(conn, _params) do
    render(conn, :home)
  end
end

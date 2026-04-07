defmodule PankoWeb.PageControllerTest do
  use PankoWeb.ConnCase

  setup %{conn: conn} do
    user = register_user()
    conn = log_in_user(conn, user)
    %{conn: conn, user: user}
  end

  test "GET / renders sessions page", %{conn: conn} do
    conn = get(conn, ~p"/")
    assert html_response(conn, 200) =~ "Sessions"
  end
end

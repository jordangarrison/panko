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

  test "GET / includes favicon link tags", %{conn: conn} do
    conn = get(conn, ~p"/")
    html = html_response(conn, 200)
    assert html =~ ~s|href="/favicon.svg"|
    assert html =~ ~s|href="/favicon.ico"|
  end
end

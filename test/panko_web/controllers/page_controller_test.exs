defmodule PankoWeb.PageControllerTest do
  use PankoWeb.ConnCase

  test "GET / renders sessions page", %{conn: conn} do
    conn = get(conn, ~p"/")
    assert html_response(conn, 200) =~ "Sessions"
  end
end

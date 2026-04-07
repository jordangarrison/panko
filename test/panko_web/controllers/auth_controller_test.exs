defmodule PankoWeb.AuthControllerTest do
  use PankoWeb.ConnCase, async: true

  describe "sign_out" do
    test "redirects to sign-in page", %{conn: conn} do
      user = register_user()
      conn = log_in_user(conn, user)

      conn = get(conn, ~p"/sign-out")
      assert redirected_to(conn) == "/sign-in"
    end
  end
end

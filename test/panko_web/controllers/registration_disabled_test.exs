defmodule PankoWeb.RegistrationDisabledTest do
  use PankoWeb.ConnCase, async: true

  describe "registration is disabled" do
    test "GET /register returns 404", %{conn: conn} do
      conn = get(conn, "/register")
      assert conn.status == 404
    end

    test "POST /auth/password/register returns 404", %{conn: conn} do
      conn =
        post(conn, "/auth/password/register", %{
          user: %{
            email: "attacker@example.com",
            password: "password123456",
            password_confirmation: "password123456"
          }
        })

      assert conn.status == 404
    end

    test "sign-in page does not contain a register link", %{conn: conn} do
      conn = get(conn, "/sign-in")
      refute conn.resp_body =~ ~r/href="\/register"/
      refute conn.resp_body =~ "Need an account?"
    end
  end
end

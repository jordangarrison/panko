defmodule Panko.ReleaseTest do
  use Panko.DataCase, async: true

  alias Panko.Accounts.User

  describe "create_admin/2" do
    test "creates a user with the given email and password" do
      assert :ok = Panko.Release.create_admin("admin@example.com", "securepassword1")

      assert {:ok, user} =
               User
               |> Ash.Query.for_read(:sign_in_with_password, %{
                 email: "admin@example.com",
                 password: "securepassword1"
               })
               |> Ash.read_one(authorize?: false)

      assert user != nil
      assert to_string(user.email) == "admin@example.com"
    end

    test "returns error for duplicate email" do
      assert :ok = Panko.Release.create_admin("dupe@example.com", "securepassword1")
      assert {:error, _reason} = Panko.Release.create_admin("dupe@example.com", "otherpassword1")
    end
  end
end

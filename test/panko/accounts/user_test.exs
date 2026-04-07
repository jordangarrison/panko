defmodule Panko.Accounts.UserTest do
  use Panko.DataCase, async: true

  alias Panko.Accounts.User

  describe "register_with_password" do
    test "creates a user with valid email and password" do
      assert {:ok, user} =
               User
               |> Ash.Changeset.for_create(:register_with_password, %{
                 email: "test@example.com",
                 password: "password123456",
                 password_confirmation: "password123456"
               })
               |> Ash.create(authorize?: false)

      assert to_string(user.email) == "test@example.com"
      assert user.hashed_password != nil
      assert user.hashed_password != "password123456"
    end

    test "rejects registration without matching password confirmation" do
      assert {:error, _} =
               User
               |> Ash.Changeset.for_create(:register_with_password, %{
                 email: "test@example.com",
                 password: "password123456",
                 password_confirmation: "differentpassword"
               })
               |> Ash.create(authorize?: false)
    end

    test "rejects duplicate email" do
      params = %{
        email: "dupe@example.com",
        password: "password123456",
        password_confirmation: "password123456"
      }

      assert {:ok, _} =
               User
               |> Ash.Changeset.for_create(:register_with_password, params)
               |> Ash.create(authorize?: false)

      assert {:error, _} =
               User
               |> Ash.Changeset.for_create(:register_with_password, params)
               |> Ash.create(authorize?: false)
    end
  end

  describe "sign_in_with_password" do
    setup do
      {:ok, user} =
        User
        |> Ash.Changeset.for_create(:register_with_password, %{
          email: "login@example.com",
          password: "password123456",
          password_confirmation: "password123456"
        })
        |> Ash.create(authorize?: false)

      %{user: user}
    end

    test "signs in with correct credentials" do
      assert {:ok, user} =
               User
               |> Ash.Query.for_read(:sign_in_with_password, %{
                 email: "login@example.com",
                 password: "password123456"
               })
               |> Ash.read_one(authorize?: false)

      assert user != nil
      assert to_string(user.email) == "login@example.com"
    end

    test "rejects incorrect password" do
      assert {:error, _} =
               User
               |> Ash.Query.for_read(:sign_in_with_password, %{
                 email: "login@example.com",
                 password: "wrongpassword"
               })
               |> Ash.read_one(authorize?: false)
    end
  end
end

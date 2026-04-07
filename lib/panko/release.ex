defmodule Panko.Release do
  @moduledoc """
  Release tasks for running migrations in production.

  Used by release overlay scripts (rel/overlays/bin/server, rel/overlays/bin/migrate).

  ## Examples

      # Run all pending migrations
      bin/panko eval "Panko.Release.migrate()"

      # Rollback to a specific version
      bin/panko eval "Panko.Release.rollback(Panko.Repo, 20240101000000)"
  """

  @app :panko

  def migrate do
    load_app()

    for repo <- repos() do
      {:ok, _, _} = Ecto.Migrator.with_repo(repo, &Ecto.Migrator.run(&1, :up, all: true))
    end
  end

  def rollback(repo, version) do
    load_app()
    {:ok, _, _} = Ecto.Migrator.with_repo(repo, &Ecto.Migrator.run(&1, :down, to: version))
  end

  @doc """
  Creates an admin user with the given email and password.

  ## Examples

      bin/panko eval "Panko.Release.create_admin(\"admin@example.com\", \"supersecretpassword\")"
  """
  def create_admin(email, password) do
    load_app()

    case Panko.Accounts.User
         |> Ash.Changeset.for_create(:register_with_password, %{
           email: email,
           password: password,
           password_confirmation: password
         })
         |> Ash.create(authorize?: false) do
      {:ok, _user} -> :ok
      {:error, error} -> {:error, error}
    end
  end

  defp repos, do: Application.fetch_env!(@app, :ecto_repos)
  defp load_app, do: Application.ensure_all_started(@app)
end

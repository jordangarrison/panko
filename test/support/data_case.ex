defmodule Panko.DataCase do
  use ExUnit.CaseTemplate

  using do
    quote do
      alias Panko.Repo
      import Panko.DataCase
    end
  end

  setup tags do
    Panko.DataCase.setup_sandbox(tags)
    :ok
  end

  def setup_sandbox(tags) do
    pid = Ecto.Adapters.SQL.Sandbox.start_owner!(Panko.Repo, shared: not tags[:async])
    on_exit(fn -> Ecto.Adapters.SQL.Sandbox.stop_owner(pid) end)
  end
end

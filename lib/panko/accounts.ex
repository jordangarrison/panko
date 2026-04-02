defmodule Panko.Accounts do
  use Ash.Domain

  resources do
    resource Panko.Accounts.User
    resource Panko.Accounts.Token
  end
end

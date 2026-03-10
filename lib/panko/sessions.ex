defmodule Panko.Sessions do
  use Ash.Domain

  resources do
    resource Panko.Sessions.Session
    resource Panko.Sessions.Block
  end
end

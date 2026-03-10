defmodule Panko.Sessions do
  use Ash.Domain

  resources do
    resource Panko.Sessions.Session
    resource Panko.Sessions.Block
    resource Panko.Sessions.SubAgent
  end
end

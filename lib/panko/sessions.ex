defmodule Panko.Sessions do
  use Ash.Domain

  resources do
    resource Panko.Sessions.Session do
      define :import_from_file, action: :import_from_file, args: [:file_path]
      define :get_session, action: :read, get_by: [:id]
      define :list_sessions, action: :list_recent
      define :list_all_sessions, action: :list_projects
    end

    resource Panko.Sessions.Block
    resource Panko.Sessions.SubAgent
  end
end

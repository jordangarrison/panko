defmodule Panko.Sharing do
  use Ash.Domain

  resources do
    resource Panko.Sharing.Share do
      define :create_share, action: :create, args: [:session_id]
      define :unpublish_share, action: :unpublish
      define :republish_share, action: :republish
      define :get_share_by_slug, action: :by_slug, args: [:slug]
      define :list_active_shares, action: :active
    end
  end
end

defmodule Panko.Sharing.Share do
  use Ash.Resource,
    domain: Panko.Sharing,
    data_layer: AshPostgres.DataLayer

  postgres do
    table "shares"
    repo Panko.Repo
  end

  attributes do
    uuid_primary_key :id

    attribute :slug, :string do
      allow_nil? false
      public? true
    end

    attribute :is_shared, :boolean do
      allow_nil? false
      default true
      public? true
    end

    attribute :expires_at, :utc_datetime do
      allow_nil? true
      public? true
    end

    attribute :shared_at, :utc_datetime do
      allow_nil? false
      public? true
    end

    attribute :unshared_at, :utc_datetime do
      allow_nil? true
      public? true
    end

    attribute :user_id, :uuid do
      allow_nil? true
      public? true
    end

    timestamps()
  end

  relationships do
    belongs_to :session, Panko.Sessions.Session do
      domain Panko.Sessions
      allow_nil? false
      public? true
    end
  end

  identities do
    identity :unique_slug, [:slug]
  end

  actions do
    defaults [:read, :destroy]

    create :create do
      primary? true
      accept [:session_id, :expires_at]
      change {Panko.Sharing.Changes.GenerateSlug, []}
      change set_attribute(:shared_at, &DateTime.utc_now/0)
    end

    update :unpublish do
      accept []
      change set_attribute(:is_shared, false)
      change set_attribute(:unshared_at, &DateTime.utc_now/0)
    end

    update :republish do
      accept []
      change set_attribute(:is_shared, true)
      change set_attribute(:unshared_at, nil)
    end

    read :by_slug do
      argument :slug, :string, allow_nil?: false
      get? true
      filter expr(slug == ^arg(:slug) and is_shared == true)
      prepare build(load: [session: [:blocks, :sub_agents]])
    end

    read :active do
      filter expr(is_shared == true)

      prepare build(
                sort: [shared_at: :desc],
                load: [:session]
              )
    end
  end
end

defmodule Panko.Sessions.Block do
  use Ash.Resource,
    domain: Panko.Sessions,
    data_layer: AshPostgres.DataLayer

  postgres do
    table "blocks"
    repo Panko.Repo

    custom_indexes do
      index [:session_id, :position], unique: true
    end
  end

  attributes do
    uuid_primary_key :id

    attribute :position, :integer do
      allow_nil? false
      public? true
    end

    attribute :block_type, Panko.Sessions.Block.Type do
      allow_nil? false
      public? true
    end

    attribute :content, :string do
      allow_nil? true
      public? true
    end

    attribute :metadata, :map do
      allow_nil? true
      public? true
    end

    attribute :timestamp, :utc_datetime do
      allow_nil? true
      public? true
    end

    create_timestamp :inserted_at
  end

  relationships do
    belongs_to :session, Panko.Sessions.Session do
      allow_nil? false
      public? true
    end
  end

  identities do
    identity :session_position, [:session_id, :position]
  end

  actions do
    defaults [:read, :destroy]

    create :create do
      primary? true
      accept [:position, :block_type, :content, :metadata, :timestamp, :session_id]
    end
  end
end

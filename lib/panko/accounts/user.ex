defmodule Panko.Accounts.User do
  use Ash.Resource,
    domain: Panko.Accounts,
    data_layer: AshPostgres.DataLayer,
    extensions: [AshAuthentication],
    authorizers: [Ash.Policy.Authorizer]

  attributes do
    uuid_primary_key :id
    attribute :email, :ci_string, allow_nil?: false, public?: true
    attribute :hashed_password, :string, allow_nil?: false, sensitive?: true
  end

  identities do
    identity :unique_email, [:email]
  end

  actions do
    defaults [:read]

    read :get_by_subject do
      description "Get a user by the subject claim in a JWT"
      argument :subject, :string, allow_nil?: false
      get? true
      prepare AshAuthentication.Preparations.FilterBySubject
    end
  end

  authentication do
    tokens do
      enabled? true
      token_resource Panko.Accounts.Token
      store_all_tokens? true
      require_token_presence_for_authentication? true

      signing_secret fn _, _ ->
        Application.fetch_env(:panko, :token_signing_secret)
      end
    end

    strategies do
      password :password do
        identity_field :email
      end
    end
  end

  postgres do
    table "users"
    repo Panko.Repo
  end

  policies do
    bypass AshAuthentication.Checks.AshAuthenticationInteraction do
      authorize_if always()
    end

    policy always() do
      forbid_if always()
    end
  end
end

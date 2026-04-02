# Ash Authentication Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add username/password authentication to Panko using Ash Authentication, replacing the API key gate with a proper login/register flow.

**Architecture:** New `Panko.Accounts` domain with `User` and `Token` resources using `ash_authentication` password strategy. `ash_authentication_phoenix` provides sign-in/register UI and router integration. Protected LiveView routes redirect unauthenticated users to `/sign-in`. Public share routes remain unauthenticated.

**Tech Stack:** ash_authentication ~> 4.0, ash_authentication_phoenix ~> 2.0, bcrypt_elixir ~> 3.0

**Spec:** `docs/superpowers/specs/2026-04-02-ash-authentication-design.md`

---

## File Map

**New files:**
- `lib/panko/accounts.ex` — Accounts domain
- `lib/panko/accounts/user.ex` — User resource with AshAuthentication
- `lib/panko/accounts/token.ex` — Token resource
- `lib/panko_web/controllers/auth_controller.ex` — Auth controller
- `lib/panko_web/live_user_auth.ex` — LiveView on_mount hook for auth redirect
- `test/panko/accounts/user_test.exs` — User resource tests
- `test/panko_web/controllers/auth_controller_test.exs` — Auth controller tests
- `test/panko_web/live/auth_redirect_test.exs` — Auth redirect integration tests

**Modified files:**
- `mix.exs:42-85` — Add dependencies
- `.formatter.exs:2` — Add import_deps
- `assets/css/app.css:4-8` — Add @source for ash_authentication_phoenix
- `config/config.exs:14` — Add Panko.Accounts to ash_domains
- `config/dev.exs` — Add token_signing_secret + debug flag
- `config/test.exs` — Add token_signing_secret
- `config/runtime.exs` — Add prod token_signing_secret
- `lib/panko/application.ex:10-19` — Add AshAuthentication.Supervisor
- `lib/panko_web/router.ex` — Replace API key pipeline with auth routes
- `test/support/conn_case.ex` — Add auth helper for authenticated tests

---

### Task 1: Add Dependencies

**Files:**
- Modify: `mix.exs:63-66`
- Modify: `.formatter.exs:2`

- [ ] **Step 1: Add auth deps to mix.exs**

In `mix.exs`, add after the existing Ash deps (line 66):

```elixir
      {:ash_authentication, "~> 4.0"},
      {:ash_authentication_phoenix, "~> 2.0"},
      {:bcrypt_elixir, "~> 3.0"},
```

- [ ] **Step 2: Update .formatter.exs**

Replace line 2:

```elixir
  import_deps: [:ash, :ash_postgres, :ash_phoenix, :ash_authentication, :ash_authentication_phoenix, :phoenix],
```

- [ ] **Step 3: Fetch dependencies**

Run: `mix deps.get`
Expected: Dependencies fetched successfully, no errors.

- [ ] **Step 4: Commit**

```bash
git add mix.exs mix.lock .formatter.exs
git commit -m "feat: add ash_authentication dependencies"
```

---

### Task 2: Update Configuration

**Files:**
- Modify: `config/config.exs:14`
- Modify: `config/dev.exs` (append)
- Modify: `config/test.exs` (append)
- Modify: `config/runtime.exs` (add to prod block)
- Modify: `assets/css/app.css:4-8`

- [ ] **Step 1: Add Accounts domain to ash_domains in config/config.exs**

Replace line 14:

```elixir
config :panko, ash_domains: [Panko.Sessions, Panko.Sharing, Panko.Accounts]
```

- [ ] **Step 2: Add token signing secret to config/dev.exs**

Append to end of file:

```elixir

# Ash Authentication
config :panko, :token_signing_secret, "dev-only-secret-must-be-at-least-32-bytes-long!!"
config :ash_authentication, debug_authentication_failures?: true
```

- [ ] **Step 3: Add token signing secret to config/test.exs**

Append to end of file:

```elixir

# Ash Authentication
config :panko, :token_signing_secret, "test-only-secret-must-be-at-least-32-bytes-long!"
```

- [ ] **Step 4: Add prod token signing secret to config/runtime.exs**

Inside the existing `if config_env() == :prod do` block, before the closing `end` (line 38), add:

```elixir
  config :panko, :token_signing_secret,
    System.get_env("PANKO_TOKEN_SIGNING_SECRET") ||
      raise "PANKO_TOKEN_SIGNING_SECRET env var is required in production"
```

- [ ] **Step 5: Add Tailwind source for ash_authentication_phoenix**

In `assets/css/app.css`, after line 7 (`@source "../../lib/panko_web";`), add:

```css
@source "../../deps/ash_authentication_phoenix";
```

- [ ] **Step 6: Verify config compiles**

Run: `mix compile`
Expected: Compiles with warnings about missing `Panko.Accounts` module (expected — we create it next).

- [ ] **Step 7: Commit**

```bash
git add config/config.exs config/dev.exs config/test.exs config/runtime.exs assets/css/app.css
git commit -m "feat: configure ash_authentication signing secrets and domains"
```

---

### Task 3: Create Accounts Domain, Token, and User Resources

**Files:**
- Create: `lib/panko/accounts.ex`
- Create: `lib/panko/accounts/token.ex`
- Create: `lib/panko/accounts/user.ex`

- [ ] **Step 1: Create the Accounts domain**

Create `lib/panko/accounts.ex`:

```elixir
defmodule Panko.Accounts do
  use Ash.Domain

  resources do
    resource Panko.Accounts.User
    resource Panko.Accounts.Token
  end
end
```

- [ ] **Step 2: Create the Token resource**

Create `lib/panko/accounts/token.ex`:

```elixir
defmodule Panko.Accounts.Token do
  use Ash.Resource,
    domain: Panko.Accounts,
    data_layer: AshPostgres.DataLayer,
    extensions: [AshAuthentication.TokenResource],
    authorizers: [Ash.Policy.Authorizer]

  postgres do
    table "tokens"
    repo Panko.Repo
  end

  policies do
    bypass AshAuthentication.Checks.AshAuthenticationInteraction do
      authorize_if always()
    end
  end
end
```

- [ ] **Step 3: Create the User resource (required for domain to compile)**

Create `lib/panko/accounts/user.ex`:

```elixir
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
```

- [ ] **Step 4: Verify all three files compile together**

Run: `mix compile --warnings-as-errors`
Expected: Compiles cleanly. All three resources must be created together because the domain references both User and Token.

- [ ] **Step 5: Commit**

```bash
git add lib/panko/accounts.ex lib/panko/accounts/token.ex lib/panko/accounts/user.ex
git commit -m "feat: add Accounts domain with User and Token resources"
```

---

### Task 4: Write User Resource Tests

**Files:**
- Create: `test/panko/accounts/user_test.exs`

- [ ] **Step 1: Write tests for registration and sign-in**

Create `test/panko/accounts/user_test.exs`:

```elixir
defmodule Panko.Accounts.UserTest do
  use Panko.DataCase, async: true

  alias Panko.Accounts.User

  describe "register_with_password" do
    test "creates a user with valid email and password" do
      assert {:ok, user} =
               User
               |> Ash.Changeset.for_create(:register_with_password, %{
                 email: "test@example.com",
                 password: "password123456",
                 password_confirmation: "password123456"
               })
               |> Ash.create(authorize?: false)

      assert user.email == "test@example.com"
      assert user.hashed_password != nil
      assert user.hashed_password != "password123456"
    end

    test "rejects registration without matching password confirmation" do
      assert {:error, _} =
               User
               |> Ash.Changeset.for_create(:register_with_password, %{
                 email: "test@example.com",
                 password: "password123456",
                 password_confirmation: "differentpassword"
               })
               |> Ash.create(authorize?: false)
    end

    test "rejects duplicate email" do
      params = %{
        email: "dupe@example.com",
        password: "password123456",
        password_confirmation: "password123456"
      }

      assert {:ok, _} =
               User
               |> Ash.Changeset.for_create(:register_with_password, params)
               |> Ash.create(authorize?: false)

      assert {:error, _} =
               User
               |> Ash.Changeset.for_create(:register_with_password, params)
               |> Ash.create(authorize?: false)
    end
  end

  describe "sign_in_with_password" do
    setup do
      {:ok, user} =
        User
        |> Ash.Changeset.for_create(:register_with_password, %{
          email: "login@example.com",
          password: "password123456",
          password_confirmation: "password123456"
        })
        |> Ash.create(authorize?: false)

      %{user: user}
    end

    test "signs in with correct credentials" do
      # sign_in_with_password is a read action — use Ash.Query, not Ash.Changeset
      assert {:ok, [user]} =
               User
               |> Ash.Query.for_read(:sign_in_with_password, %{
                 email: "login@example.com",
                 password: "password123456"
               })
               |> Ash.read(authorize?: false)

      assert user.email == "login@example.com"
    end

    test "rejects incorrect password" do
      assert {:error, _} =
               User
               |> Ash.Query.for_read(:sign_in_with_password, %{
                 email: "login@example.com",
                 password: "wrongpassword"
               })
               |> Ash.read(authorize?: false)
    end
  end
end
```

- [ ] **Step 2: Run the tests — they should fail (no migrations yet)**

Run: `mix test test/panko/accounts/user_test.exs --max-failures 1 2>&1 | tail -20`
Expected: Failure due to missing `users` table.

- [ ] **Step 3: Generate and run migrations**

Run:
```bash
mix ash.codegen add_authentication
mix ash.migrate
MIX_ENV=test mix ash.migrate
```
Expected: Migration files created in `priv/repo/migrations/`. Migrations run successfully.

- [ ] **Step 4: Run the tests again**

Run: `mix test test/panko/accounts/user_test.exs`
Expected: All tests pass. If any fail, read the error and fix the User resource or test accordingly.

Note: `sign_in_with_password` is a read action — use `Ash.Query.for_read/3` + `Ash.read/2`, not `Ash.Changeset`. If the `Ash.Query` approach doesn't work with the strategy's internal preparations, try `AshAuthentication.Strategy.action/3` or consult `mix usage_rules.search_docs "sign_in_with_password" -p ash_authentication`.

- [ ] **Step 5: Commit**

```bash
git add test/panko/accounts/user_test.exs priv/repo/migrations/
git commit -m "test: add User resource registration and sign-in tests"
```

---

### Task 5: Add AshAuthentication Supervisor and LiveUserAuth Hook

**Files:**
- Modify: `lib/panko/application.ex:10-19`
- Create: `lib/panko_web/live_user_auth.ex`

- [ ] **Step 1: Add AshAuthentication.Supervisor to application.ex**

In `lib/panko/application.ex`, add `{AshAuthentication.Supervisor, otp_app: :panko}` to the children list, after `Panko.Repo` (line 13) and before `DNSCluster`:

```elixir
    children =
      [
        PankoWeb.Telemetry,
        Panko.Repo,
        {AshAuthentication.Supervisor, otp_app: :panko},
        {DNSCluster, query: Application.get_env(:panko, :dns_cluster_query) || :ignore},
        {Phoenix.PubSub, name: Panko.PubSub},
        {Oban, Application.fetch_env!(:panko, Oban)},
        maybe_session_watcher(),
        PankoWeb.Endpoint
      ]
      |> Enum.reject(&is_nil/1)
```

- [ ] **Step 2: Create LiveUserAuth on_mount hook**

Create `lib/panko_web/live_user_auth.ex`:

```elixir
defmodule PankoWeb.LiveUserAuth do
  @moduledoc """
  LiveView on_mount hook that redirects unauthenticated users to sign-in.
  """
  import Phoenix.LiveView
  import Phoenix.Component

  def on_mount(:live_user_required, _params, _session, socket) do
    if socket.assigns[:current_user] do
      {:cont, socket}
    else
      {:halt, redirect(socket, to: ~p"/sign-in")}
    end
  end
end
```

- [ ] **Step 3: Verify it compiles**

Run: `mix compile --warnings-as-errors`
Expected: Compiles cleanly.

- [ ] **Step 4: Commit**

```bash
git add lib/panko/application.ex lib/panko_web/live_user_auth.ex
git commit -m "feat: add AshAuthentication supervisor and LiveUserAuth hook"
```

---

### Task 6: Create Auth Controller

**Files:**
- Create: `lib/panko_web/controllers/auth_controller.ex`

- [ ] **Step 1: Create the auth controller**

Create `lib/panko_web/controllers/auth_controller.ex`:

```elixir
defmodule PankoWeb.AuthController do
  use PankoWeb, :controller
  use AshAuthentication.Phoenix.Controller

  def success(conn, _activity, user, _token) do
    conn
    |> store_in_session(user)
    |> assign(:current_user, user)
    |> redirect(to: ~p"/")
  end

  def failure(conn, _activity, _reason) do
    conn
    |> put_flash(:error, "Authentication failed")
    |> redirect(to: ~p"/sign-in")
  end

  def sign_out(conn, _params) do
    conn
    |> clear_session()
    |> redirect(to: ~p"/sign-in")
  end
end
```

- [ ] **Step 2: Verify it compiles**

Run: `mix compile --warnings-as-errors`
Expected: Compiles cleanly.

- [ ] **Step 3: Commit**

```bash
git add lib/panko_web/controllers/auth_controller.ex
git commit -m "feat: add auth controller for sign-in/out callbacks"
```

---

### Task 7: Update Router

**Files:**
- Modify: `lib/panko_web/router.ex`

This is the most critical change. We replace the API key pipeline with Ash Authentication's session-based auth.

- [ ] **Step 1: Rewrite the router**

Replace the entire contents of `lib/panko_web/router.ex`:

```elixir
defmodule PankoWeb.Router do
  use PankoWeb, :router
  use AshAuthentication.Phoenix.Router

  pipeline :browser do
    plug :accepts, ["html"]
    plug :fetch_session
    plug :fetch_live_flash
    plug :put_root_layout, html: {PankoWeb.Layouts, :root}
    plug :protect_from_forgery
    plug :put_secure_browser_headers
    plug :load_from_session
  end

  pipeline :api do
    plug :accepts, ["json"]
  end

  # Public auth routes (sign in, register, sign out, auth callbacks)
  scope "/", PankoWeb do
    pipe_through :browser

    sign_in_route(register_path: "/register", auth_routes_prefix: "/auth")
    sign_out_route AuthController
    auth_routes AuthController, Panko.Accounts.User, path: "/auth"
  end

  # Protected routes — require authenticated user
  scope "/", PankoWeb do
    pipe_through :browser

    ash_authentication_live_session :authenticated,
      otp_app: :panko,
      on_mount_prepend: [{PankoWeb.LiveUserAuth, :live_user_required}],
      layout: {PankoWeb.Layouts, :app} do
      live "/", SessionsLive, :index
      live "/sessions/:id", SessionLive, :show
    end
  end

  # Public share routes — no auth
  scope "/s", PankoWeb do
    pipe_through :browser

    live_session :public do
      live "/:slug", ShareLive, :show
    end
  end
end
```

- [ ] **Step 2: Verify it compiles**

Run: `mix compile --warnings-as-errors`
Expected: Compiles cleanly. If there are issues with the router macros, check `mix usage_rules.search_docs "sign_in_route" -p ash_authentication_phoenix` for correct signatures.

- [ ] **Step 3: Run existing tests to check for regressions**

Run: `mix test`
Expected: Existing tests may need updates if they rely on the old router structure. Note failures but don't fix yet — that's the next task.

- [ ] **Step 4: Commit**

```bash
git add lib/panko_web/router.ex
git commit -m "feat: replace API key auth with Ash Authentication routes"
```

---

### Task 8: Add Auth Test Helpers and Update ConnCase

**Files:**
- Modify: `test/support/conn_case.ex`
- Create: `test/panko_web/controllers/auth_controller_test.exs`
- Create: `test/panko_web/live/auth_redirect_test.exs`

- [ ] **Step 1: Add auth helper to ConnCase**

Replace `test/support/conn_case.ex` with:

```elixir
defmodule PankoWeb.ConnCase do
  @moduledoc """
  This module defines the test case to be used by
  tests that require setting up a connection.
  """

  use ExUnit.CaseTemplate

  using do
    quote do
      @endpoint PankoWeb.Endpoint

      use PankoWeb, :verified_routes

      import Plug.Conn
      import Phoenix.ConnTest
      import PankoWeb.ConnCase
    end
  end

  setup tags do
    Panko.DataCase.setup_sandbox(tags)
    {:ok, conn: Phoenix.ConnTest.build_conn()}
  end

  @doc """
  Creates a registered user and returns the user struct.
  """
  def register_user(attrs \\ %{}) do
    params =
      Map.merge(
        %{
          email: "user#{System.unique_integer()}@example.com",
          password: "password123456",
          password_confirmation: "password123456"
        },
        attrs
      )

    {:ok, user} =
      Panko.Accounts.User
      |> Ash.Changeset.for_create(:register_with_password, params)
      |> Ash.create(authorize?: false)

    user
  end

  @doc """
  Logs in a user by putting auth info in the session.
  Returns the updated conn.

  Uses AshAuthentication.Plug.Helpers.store_in_session/2 which is the same
  function used by the AuthController on successful login.
  """
  def log_in_user(conn, user) do
    conn
    |> Phoenix.ConnTest.init_test_session(%{})
    |> AshAuthentication.Plug.Helpers.store_in_session(user)
  end
end
```

- [ ] **Step 2: Write auth controller tests**

Create `test/panko_web/controllers/auth_controller_test.exs`:

```elixir
defmodule PankoWeb.AuthControllerTest do
  use PankoWeb.ConnCase, async: true

  describe "sign_out" do
    test "redirects to sign-in page", %{conn: conn} do
      user = register_user()
      conn = log_in_user(conn, user)

      conn = get(conn, ~p"/sign-out")
      assert redirected_to(conn) == "/sign-in"
    end
  end
end
```

- [ ] **Step 3: Write auth redirect tests**

Create `test/panko_web/live/auth_redirect_test.exs`:

```elixir
defmodule PankoWeb.AuthRedirectTest do
  use PankoWeb.ConnCase, async: true
  import Phoenix.LiveViewTest

  describe "unauthenticated access" do
    test "redirects / to sign-in", %{conn: conn} do
      assert {:error, {:redirect, %{to: "/sign-in"}}} = live(conn, ~p"/")
    end

    test "redirects /sessions/:id to sign-in", %{conn: conn} do
      assert {:error, {:redirect, %{to: "/sign-in"}}} =
               live(conn, ~p"/sessions/00000000-0000-0000-0000-000000000000")
    end
  end

  describe "public share access" do
    test "/s/:slug does not redirect to sign-in", %{conn: conn} do
      # This will fail with a different error (share not found), not an auth redirect
      result = live(conn, ~p"/s/nonexistent")

      case result do
        {:error, {:redirect, %{to: "/sign-in"}}} ->
          flunk("Share route should not redirect to sign-in")

        _ ->
          # Any other result (including crashes for missing share) is acceptable
          assert true
      end
    end
  end
end
```

- [ ] **Step 4: Run all tests**

Run: `mix test`
Expected: All tests pass. If `log_in_user` doesn't work with `AshAuthentication.Jwt.token_for_user/1`, consult `mix usage_rules.search_docs "token_for_user" -p ash_authentication` and adjust the helper. The key is getting user data into the session the way `load_from_session` expects it.

- [ ] **Step 5: Fix any failing existing tests**

If existing LiveView tests fail because they now get redirected to sign-in, update them to use `log_in_user(conn, user)` before making requests. Check each failing test and add authentication.

- [ ] **Step 6: Run full test suite and format**

Run:
```bash
mix format
mix test
```
Expected: All tests pass, code formatted.

- [ ] **Step 7: Commit**

```bash
git add test/support/conn_case.ex test/panko_web/controllers/auth_controller_test.exs test/panko_web/live/auth_redirect_test.exs
git commit -m "test: add auth test helpers and authentication tests"
```

---

### Task 9: Fix Remaining Test Failures and Final Verification

**Files:**
- Modify: any existing test files that fail due to auth requirement

- [ ] **Step 1: Run full test suite**

Run: `mix test 2>&1 | tail -30`
Identify any failures.

- [ ] **Step 2: Fix each failing test**

For each failing test that gets an auth redirect:
1. Add `user = register_user()` in setup
2. Use `conn = log_in_user(conn, user)` before the test's HTTP request
3. Re-run just that test file to verify the fix

Known files that will likely need updates:
- `test/panko_web/controllers/page_controller_test.exs` — `GET /` now redirects without auth
- Any LiveView test files that hit protected routes without authentication

- [ ] **Step 3: Run full validation suite**

Run:
```bash
mix compile --warnings-as-errors
mix format --check-formatted
mix test
```
Expected: All three commands pass cleanly.

- [ ] **Step 4: Commit any test fixes**

```bash
git add test/
git commit -m "fix: update existing tests for authentication requirement"
```

---

### Task 10: Final Cleanup and Verification

- [ ] **Step 1: Run full precommit check**

Run: `mix precommit`
Expected: Compile, format, and test all pass.

- [ ] **Step 2: Verify sign-in page renders (manual or via test)**

Start the server briefly or write a quick test:

```bash
mix test test/panko_web/live/auth_redirect_test.exs test/panko_web/controllers/auth_controller_test.exs test/panko/accounts/user_test.exs -v
```

Expected: All auth-related tests pass with verbose output showing test names.

- [ ] **Step 3: Commit if any cleanup was needed**

Only if changes were made, stage specific files:
```bash
git add lib/ test/ config/
git commit -m "chore: auth implementation cleanup"
```

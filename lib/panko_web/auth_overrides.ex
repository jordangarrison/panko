defmodule PankoWeb.AuthOverrides do
  use AshAuthentication.Phoenix.Overrides

  override AshAuthentication.Phoenix.Components.Password do
    set :register_toggle_text, nil
  end
end

defmodule Panko.Sharing.Changes.GenerateSlug do
  use Ash.Resource.Change

  @slug_length 8
  @alphabet ~c"abcdefghijklmnopqrstuvwxyz0123456789"

  @impl true
  def change(changeset, _opts, _context) do
    if Ash.Changeset.get_attribute(changeset, :slug) do
      changeset
    else
      slug = generate_slug()
      Ash.Changeset.force_change_attribute(changeset, :slug, slug)
    end
  end

  defp generate_slug do
    for _ <- 1..@slug_length, into: "" do
      <<Enum.random(@alphabet)>>
    end
  end
end

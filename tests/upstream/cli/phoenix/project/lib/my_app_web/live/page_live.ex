defmodule MyAppWeb.PageLive do
  use MyAppWeb, :live_view

  def render(assigns) do
    ~H"""
    <section class="grid grid-cols-1 gap-6 md:grid-cols-2" id="page">
      <p class={"italic #{if @big, do: "text-4xl", else: "text-lg"}"}>Hello</p>
    </section>
    """
  end

  defp badge_class(:ok), do: "bg-green-100 text-green-800"
  defp badge_class(_), do: ~w(bg-red-100 text-red-800)
end

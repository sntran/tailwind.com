defmodule MyAppWeb.CoreComponents do
  use Phoenix.Component

  attr :type, :string, default: nil
  attr :class, :string, default: nil
  attr :variant, :string, values: ~w(primary secondary)
  slot :inner_block, required: true

  def button(assigns) do
    ~H"""
    <button
      type={@type}
      class={[
        "phx-submit-loading:opacity-75 rounded-lg bg-zinc-900 hover:bg-zinc-700 py-2 px-3",
        "text-sm font-semibold leading-6 text-white active:text-white/80",
        @variant == "primary" && "bg-indigo-600",
        @class
      ]}
    >
      {render_slot(@inner_block)}
    </button>
    """
  end

  attr :kind, :atom, values: [:info, :error]

  def flash(assigns) do
    ~H"""
    <div
      role="alert"
      class={[
        "fixed top-2 right-2 mr-2 w-80 sm:w-96 z-50 rounded-lg p-3 ring-1",
        @kind == :info && "bg-emerald-50 text-emerald-800 ring-emerald-500 fill-cyan-900",
        @kind == :error && "bg-rose-50 text-rose-900 shadow-md ring-rose-500 fill-rose-900"
      ]}
    >
      <p class="flex items-center gap-1.5 text-sm font-semibold leading-6">Title</p>
    </div>
    """
  end

  def icon(%{name: "hero-" <> _} = assigns) do
    ~H"""
    <span class={[@name, @class, "size-5 shrink-0"]} />
    """
  end

  def show(js \\ %JS{}, selector) do
    JS.show(js,
      to: selector,
      time: 300,
      transition:
        {"transition-all ease-out duration-300",
         "opacity-0 translate-y-4 sm:translate-y-0 sm:scale-95",
         "opacity-100 translate-y-0 sm:scale-100"}
    )
  end
end

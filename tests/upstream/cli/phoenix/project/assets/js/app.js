import "phoenix_html"
import {Socket} from "phoenix"
import {LiveSocket} from "phoenix_live_view"

let Hooks = {}
Hooks.Flash = {
  mounted() {
    this.el.classList.add("transition-opacity", "duration-300")
    setTimeout(() => this.el.classList.add("opacity-0"), 5000)
  }
}
let liveSocket = new LiveSocket("/live", Socket, {hooks: Hooks})
liveSocket.connect()

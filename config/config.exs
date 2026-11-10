import Config

# The tests of this project set the version and the path themselves.
if config_env() == :test do
  config :tailwind, version_check: false
end

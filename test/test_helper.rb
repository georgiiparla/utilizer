require 'json'
require "minitest/autorun"
require "minitest/emoji"

CONFIG = JSON.parse(
        File.read(File.join(__dir__, "fixtures", "config.json")),
        symbolize_names: true
).freeze

$LOAD_PATH.unshift File.join(Dir.pwd, "app")
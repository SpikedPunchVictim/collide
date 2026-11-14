# Reads a YAML file (argv[0]) or a JSON array of YAML docs (argv[0] = "-json", stdin), prints JSON of what Psych
# (libyaml, YAML 1.1 resolution) makes of it. Used as a YAML 1.1 oracle. Ruby's system 2.6 / Psych 3.1.
require 'yaml'
require 'json'
require 'date'
require 'time'

def norm(v)
  case v
  when Date, Time then { '__t' => v.class.name, 'v' => v.to_s }
  when Float then v.nan? ? { '__t' => 'Float', 'v' => 'NaN' } : (v.infinite? ? { '__t' => 'Float', 'v' => v.to_s } : v)
  when Array then v.map { |x| norm(x) }
  when Hash then v.map { |k, x| [k.is_a?(String) ? k : "#{k.class}:#{k.inspect}", norm(x)] }.to_h
  when Symbol then { '__t' => 'Symbol', 'v' => v.to_s }
  else v
  end
end

def load(s)
  norm(YAML.safe_load(s, [Date, Time, Symbol], [], true))
rescue StandardError, SyntaxError => e
  { '__error' => "#{e.class}: #{e.message}" }
end

if ARGV[0] == '-json'
  docs = JSON.parse($stdin.read)
  puts JSON.generate(docs.map { |d| load(d) })
else
  puts JSON.generate(load(File.read(ARGV[0])))
end

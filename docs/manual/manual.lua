-- pandoc's filter for the user manual, see utils/build-manual.sh:
-- - GitHub's alerts, block quotes that start with [!NOTE], [!TIP], [!IMPORTANT], [!WARNING] or
--   [!CAUTION], become the boxes of template.tex, with a title in the manual's language. GitHub
--   shows the same Markdown as its own alerts.
-- - Before each table, room is asked for all of it, so a table of up to half a page isn't split
--   from its heading or over two pages.

local titles = {
  en = { NOTE = "Note", TIP = "Tip", IMPORTANT = "Important", WARNING = "Warning", CAUTION = "Caution" },
  de = { NOTE = "Hinweis", TIP = "Tipp", IMPORTANT = "Wichtig", WARNING = "Achtung", CAUTION = "Vorsicht" },
}

local lang = "en"

function Meta(meta)
  if meta.lang then
    lang = pandoc.utils.stringify(meta.lang):sub(1, 2)
  end
end

function BlockQuote(quote)
  local first = quote.content[1]
  if not first or first.t ~= "Para" or not first.content[1] or first.content[1].t ~= "Str" then
    return nil
  end
  local kind = first.content[1].text:match("^%[!(%u+)%]$")
  local title = kind and (titles[lang] or titles.en)[kind]
  if not title then
    return nil
  end
  -- The marker and the line break after it.
  first.content:remove(1)
  while first.content[1] and (first.content[1].t == "SoftBreak" or first.content[1].t == "Space"
      or first.content[1].t == "LineBreak") do
    first.content:remove(1)
  end
  if #first.content == 0 then
    quote.content:remove(1)
  end
  local blocks = { pandoc.RawBlock("latex", "\\begin{callout}{" .. title .. "}") }
  for _, block in ipairs(quote.content) do
    table.insert(blocks, block)
  end
  table.insert(blocks, pandoc.RawBlock("latex", "\\end{callout}"))
  return blocks
end

-- About how many characters of the small type of tables fit on a line as wide as the text.
local LINE = 100

-- The lines a row takes: as many as its longest cell, as its text wraps in its column.
local function row_lines(row, widths)
  local lines = 1
  for i, cell in ipairs(row.cells) do
    local width = widths[i] or (1 / #widths)
    local chars = utf8.len(pandoc.utils.stringify(cell.contents)) or 0
    lines = math.max(lines, math.ceil(chars / math.max(width * LINE - 4, 8)))
  end
  return lines
end

-- Asks for as much room as the table needs, so it isn't split from its heading or over two
-- pages, unless it takes more than half a page anyway.
local function needspace(tbl)
  local widths = {}
  for i, spec in ipairs(tbl.colspecs) do
    local width = spec[2]
    if type(width) == "number" then
      widths[i] = width
    end
  end
  for i = 1, #tbl.colspecs do
    widths[i] = widths[i] or (1 / #tbl.colspecs)
  end
  local lines = 3
  for _, row in ipairs(tbl.head.rows) do
    lines = lines + row_lines(row, widths)
  end
  for _, body in ipairs(tbl.bodies) do
    for _, row in ipairs(body.body) do
      lines = lines + row_lines(row, widths) * 1.15
    end
  end
  lines = math.min(math.ceil(lines), 30)
  return pandoc.RawBlock("latex", "\\needspace{" .. lines .. "\\baselineskip}")
end

-- The room goes before the heading of a table's section, if the table follows it at once, so
-- the heading moves to the next page with the table.
function Blocks(blocks)
  local result = pandoc.List()
  for _, block in ipairs(blocks) do
    if block.t == "Table" then
      local last = result[#result]
      if last and last.t == "Header" then
        result:insert(#result, needspace(block))
      else
        result:insert(needspace(block))
      end
    end
    result:insert(block)
  end
  return result
end

return { { Meta = Meta }, { BlockQuote = BlockQuote }, { Blocks = Blocks } }

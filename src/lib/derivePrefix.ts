const MAX_LEN = 6
const VOWELS = new Set(['A', 'E', 'I', 'O', 'U'])

function abbreviateWord(word: string, maxLen: number): string {
  if (word.length <= maxLen) return word
  const first = word[0]
  const consonants = [...word.slice(1)].filter((c) => !VOWELS.has(c))
  const vowels = [...word.slice(1)].filter((c) => VOWELS.has(c))
  const rest = [...consonants, ...vowels].slice(0, maxLen - 1)
  return (first + rest.join('')).slice(0, maxLen)
}

export function derivePrefix(projectName: string): string {
  const words = projectName
    .split(/[^A-Za-z0-9]+/)
    .filter(Boolean)
    .map((w) => w.toUpperCase())

  if (words.length === 0) return 'PROJECT'

  let result: string
  if (words.length === 1) {
    result = abbreviateWord(words[0], MAX_LEN)
  } else {
    const charsPerWord = Math.ceil(MAX_LEN / words.length)
    result = words.map((w) => abbreviateWord(w, charsPerWord)).join('').slice(0, MAX_LEN)
  }

  return result || 'PROJECT'
}

use std::{
    str::FromStr,
    error::Error as StdError,
    fmt
};

/// Defines how an operation on an `Args` method failed.
#[derive(Debug)]
pub enum Error<E: StdError> {
    /// "END-OF-STRING", more precisely, there isn't anything to parse anymore.
    Eos,
    /// A parsing operation failed; the error in it can be of any returned from the `FromStr`
    /// trait.
    Parse(E),
}

impl<E: StdError> From<E> for Error<E> {
    fn from(e: E) -> Self {
        Error::Parse(e)
    }
}

impl<E: StdError> StdError for Error<E> {
    fn description(&self) -> &str {
        use self::Error::*;

        match *self {
            Eos => "end-of-string",
            Parse(ref e) => e.description(),
        }
    }

    fn cause(&self) -> Option<&StdError> {
        use self::Error::*;

        match *self {
            Parse(ref e) => Some(e),
            _ => None,
        }
    }
}

impl<E: StdError> fmt::Display for Error<E> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::Error::*;

        match *self {
            Eos => write!(f, "end of string"),
            Parse(ref e) => fmt::Display::fmt(&e, f),
        }
    }
}

type Result<T, E> = ::std::result::Result<T, Error<E>>;

fn find_end(s: &str, i: usize) -> Option<usize> {
    if i > s.len() {
        return None;
    }

    let mut end = i + 1;
    while !s.is_char_boundary(end) {
        end += 1;
    }

    Some(end)
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum TokenKind {
    Delimiter,
    Argument,
    QuotedArgument,
    Eof,
}

#[derive(Debug)]
struct Token<'a> {
    lit: &'a str,
    kind: TokenKind,
    pos: usize,
}

impl<'a> Token<'a> {
    fn new(kind: TokenKind, lit: &'a str, pos: usize) -> Self {
        Token { kind, lit, pos }
    }

    fn empty() -> Self {
        Token {
            kind: TokenKind::Eof,
            lit: "",
            pos: !0,
        }
    }
}

#[derive(Debug, Clone)]
struct TokenOwned {
    kind: TokenKind,
    lit: String,
    pos: usize,
}

impl<'a> Token<'a> {
    fn to_owned(&self) -> TokenOwned {
        TokenOwned {
            kind: self.kind,
            lit: self.lit.to_string(),
            pos: self.pos,
        }
    }
}

impl PartialEq<TokenKind> for TokenOwned {
    fn eq(&self, other: &TokenKind) -> bool {
        self.kind == *other
    }
}

#[derive(Debug)]
struct Lexer<'a> {
    msg: &'a str,
    delims: &'a [char],
    offset: usize,
}

impl<'a> Lexer<'a> {
    fn new(msg: &'a str, delims: &'a [char]) -> Self {
        Lexer {
            msg,
            delims,
            offset: 0,
        }
    }

    fn at_end(&self) -> bool {
        self.offset >= self.msg.len()
    }

    fn current(&self) -> Option<&str> {
        if self.at_end() {
            return None;
        }

        let start = self.offset;

        let end = find_end(&self.msg, self.offset)?;

        Some(&self.msg[start..end])
    }

    fn next(&mut self) -> Option<()> {
        self.offset += self.current()?.len();

        Some(())
    }

    fn commit(&mut self) -> Token<'a> {
        if self.at_end() {
            return Token::empty();
        }

        if self.current().unwrap().contains(self.delims) {
            let start = self.offset;
            self.next();
            return Token::new(TokenKind::Delimiter, &self.msg[start..self.offset], start);
        }

        if self.current().unwrap() == "\"" {
            let start = self.offset;
            self.next();

            while !self.at_end() && self.current().unwrap() != "\"" {
                self.next();
            }

            self.next();
            let end = self.offset;

            return if self.at_end() && &self.msg[end-1..end] != "\"" {
                // invalid, missing an end quote; view it as a normal argument instead.
                Token::new(TokenKind::Argument, &self.msg[start..], start)
            } else {
                Token::new(TokenKind::QuotedArgument, &self.msg[start..end], start)
            };
        }

        let start = self.offset;

        while !self.at_end() && !self.current().unwrap().contains(self.delims) {
            self.next();
        }

        Token::new(TokenKind::Argument, &self.msg[start..self.offset], start)
    }
}

/// A utility struct for handling "arguments" of a command.
///
/// An "argument" is a part of the message up that ends at one of the specified delimiters, or the end of the message.
/// 
/// # Example
/// 
/// ```rust
/// use serenity::framework::standard::Args;
/// 
/// let mut args = Args::new("hello world!", &[" ".to_string()]); // A space is our delimiter.
/// 
/// // Parse our argument as a `String` and assert that it's the "hello" part of the message.
/// assert_eq!(args.single::<String>().unwrap(), "hello");
/// // Same here.
/// assert_eq!(args.single::<String>().unwrap(), "world!");
/// 
/// ```
/// 
/// We can also parse "quoted arguments" (no pun intended): 
/// 
/// ```rust
/// use serenity::framework::standard::Args;
/// 
/// // Let us imagine this scenario:
/// // You have a `photo` command that grabs the avatar url of a user. This command accepts names only.
/// // Now, one of your users wants the avatar of a user named Princess Zelda.
/// // Problem is, her name contains a space; our delimiter. This would result in two arguments, "Princess" and "Zelda".
/// // So how should we get around this? Through quotes! By surrounding her name in them we can perceive it as one single argument. 
/// let mut args = Args::new("\"Princess Zelda\""#, &[" ".to_string()]);
/// 
/// // Hooray!
/// assert_eq!(args.single_quoted::<String>().unwrap(), "Princess Zelda");
/// ```
/// 
/// In case of a mistake, we can go back in time... er i mean, one step (or entirely):
/// 
/// ```rust
/// use serenity::framework::standard::Args;
/// 
/// let mut args = Args::new("4 20", &[" ".to_string()]);
/// 
/// assert_eq!(args.single::<u32>().unwrap(), 4);
/// 
/// // Oh wait, oops, meant to double the 4.
/// // But i won't able to access it now...
/// // oh wait, i can `rewind`.
/// args.rewind();
/// 
/// assert_eq!(args.single::<u32>().unwrap() * 2, 8);
/// 
/// // And the same for the 20
/// assert_eq!(args.single::<u32>().unwrap() * 2, 40);
/// 
/// // WAIT, NO. I wanted to concatenate them into a "420" string...
/// // Argh, what should i do now????
/// // ....
/// // oh, `restore`
/// args.restore();
/// 
/// let res = format!("{}{}", args.single::<String>().unwrap(), args.single::<String>().unwrap());
/// 
/// // Yay.
/// assert_eq!(res, "420");
/// ```
/// 
/// **A category of methods not showcased as an example**: `*_n` methods. 
/// Their docs are good enough to explain themselves. (+ don't want to clutter this doc with another Example)
#[derive(Clone, Debug)]
pub struct Args {
    message: String,
    args: Vec<TokenOwned>,
    offset: usize,
}

impl Args {
    pub fn new(message: &str, possible_delimiters: &[String]) -> Self {
        let delims = possible_delimiters
            .iter()
            .filter(|d| message.contains(d.as_str()))
            .flat_map(|s| s.chars())
            .collect::<Vec<_>>();

        let mut lex = Lexer::new(message, &delims);

        let mut args = Vec::new();

        while !lex.at_end() {
            let token = lex.commit();

            if token.kind == TokenKind::Delimiter {
                continue;
            }

            args.push(token.to_owned());
        }

        Args {
            args,
            message: message.to_string(),
            offset: 0,
        }
    }

    /// Parses the current argument and advances.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use serenity::framework::standard::Args;
    ///
    /// let mut args = Args::new("42 69", &[" ".to_string()]);
    ///
    /// assert_eq!(args.single::<u32>().unwrap(), 42);
    /// 
    /// // `42` is now out of the way, next we have `69`
    /// assert_eq!(args.single::<u32>().unwrap(), 69);
    /// ```
    pub fn single<T: FromStr>(&mut self) -> Result<T, T::Err>
        where T::Err: StdError {
        if self.is_empty() {
            return Err(Error::Eos);
        }

        let cur = &self.args[self.offset];

        let parsed = T::from_str(&cur.lit)?;
        self.offset += 1;
        Ok(parsed)
    }

    /// Like [`single`], but doesn't advance.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use serenity::framework::standard::Args;
    ///
    /// let args = Args::new("42 69", &[" ".to_string()]);
    ///
    /// assert_eq!(args.single_n::<u32>().unwrap(), 42);
    /// assert_eq!(args, "42 69");
    /// ```
    ///
    /// [`single`]: #method.single
    pub fn single_n<T: FromStr>(&self) -> Result<T, T::Err>
        where T::Err: StdError {
        if self.is_empty() {
            return Err(Error::Eos);
        }

        let cur = &self.args[self.offset];

        Ok(T::from_str(&cur.lit)?)
    }

    /// Gets original message passed to the command.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use serenity::framework::standard::Args;
    ///
    /// let args = Args::new("42 69", &[" ".to_string()]);
    ///
    /// assert_eq!(args.full(), "42 69");
    /// ```
    pub fn full(&self) -> &str { 
        &self.message 
    }

    /// Gets the original message passed to the command, 
    /// but without quotes (if both starting and ending quotes are present, otherwise returns as is).
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use serenity::framework::standard::Args;
    ///
    /// let args = Args::new("\"42 69\"", &[" ".to_string()]);
    ///
    /// assert_eq!(args.full_quoted(), "42 69");
    /// ```
    /// 
    /// ```rust
    /// use serenity::framework::standard::Args;
    /// 
    /// let args = Args::new("\"42 69", &[" ".to_string()]);
    ///
    /// assert_eq!(args.full_quoted(), "\"42 69");
    /// ```
    /// 
    /// ```rust
    /// use serenity::framework::standard::Args;
    /// 
    /// let args = Args::new("42 69\"", &[" ".to_string()]);
    ///
    /// assert_eq!(args.full_quoted(), "42 69\"");
    /// ```
    pub fn full_quoted(&self) -> &str {
        let s = &self.message;

        if !s.starts_with('"') {
            return s;
        }
    
        let end = s.rfind('"');
        if end.is_none() {
            return s;
        }

        let end = end.unwrap();
    
        // If it got the quote at the start, then there's no closing quote.
        if end == 0 {
            return s;
        }
    
        &s[1..end]
    }

    /// Returns the message starting from the token in the current argument offset; the "rest" of the message.
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use serenity::framework::standard::Args;
    /// 
    /// let mut args = Args::new("42 69 91", &[" ".to_string()]);
    ///
    /// assert_eq!(args.rest(), "42 69 91");
    /// 
    /// args.skip();
    /// 
    /// assert_eq!(args.rest(), "69 91");
    /// 
    /// args.skip();
    /// 
    /// assert_eq!(args.rest(), "91");
    /// 
    /// args.skip();
    /// 
    /// assert_eq!(args.rest(), "");
    /// ```
    pub fn rest(&self) -> &str {
        if self.is_empty() {
            return "";
        }
        
        let args = &self.args[self.offset..];

        if let Some(token) = args.get(0) {
            &self.message[token.pos..]
        } else {
            &self.message[..]
        }
    }

    /// The full amount of recognised arguments.
    ///
    /// **Note**:
    /// This never changes. Except for [`find`], which upon success, subtracts the length by 1. (e.g len of `3` becomes `2`) 
    ///
    /// # Examples
    ///
    /// ```rust
    /// use serenity::framework::standard::Args;
    ///
    /// let mut args = Args::new("42 69", &[" ".to_string()]);
    ///
    /// assert_eq!(args.len(), 2); // `2` because `["42", "69"]`
    /// ```
    ///
    /// [`find`]: #method.find
    pub fn len(&self) -> usize {
        self.args.len()
    }

    /// Amount of arguments still available.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use serenity::framework::standard::Args;
    ///
    /// let mut args = Args::new("42 69", &[" ".to_string()]);
    ///
    /// assert_eq!(args.remaining(), 2);
    /// 
    /// args.skip();
    /// 
    /// assert_eq!(args.remaining(), 1);
    /// ```
    pub fn remaining(&self) -> usize {
        if self.is_empty() {
            return 0;
        }

        self.len() - self.offset
    }

    /// Returns true if there are no arguments left.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use serenity::framework::standard::Args;
    ///
    /// let mut args = Args::new("", &[" ".to_string()]);
    ///
    /// assert!(args.is_empty()); // `true` because passed message is empty thus no arguments.
    /// ```
    pub fn is_empty(&self) -> bool {
        self.offset >= self.args.len()
    }

    /// Go one step behind.
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use serenity::framework::standard::Args;
    /// 
    /// let mut args = Args::new("42 69", &[" ".to_string()]);
    ///
    /// assert_eq!(args.single::<u32>().unwrap(), 42);
    /// 
    /// // By this point, we can only parse 69 now.
    /// // However, with the help of `rewind`, we can mess with 42 again.
    /// args.rewind();
    /// 
    /// assert_eq!(args.single::<u32>().unwrap() * 2, 84);
    /// ```
    #[inline]
    pub fn rewind(&mut self) {
        if self.offset == 0 {
            return;
        }

        self.offset -= 1;
    }

    /// Go back to the starting point.
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use serenity::framework::standard::Args;
    /// 
    /// let mut args = Args::new("42 69 95", &[" ".to_string()]);
    ///
    /// // Let's parse 'em numbers!
    /// assert_eq!(args.single::<u32>().unwrap(), 42);
    /// assert_eq!(args.single::<u32>().unwrap(), 69);
    /// assert_eq!(args.single::<u32>().unwrap(), 95);
    /// 
    /// // Oh, no! I actually wanted to multiply all of them by 2! 
    /// // I don't want to call `rewind` 3 times manually....
    /// // Wait, I could just go entirely back! 
    /// args.restore();
    /// 
    /// assert_eq!(args.single::<u32>().unwrap() * 2, 84);
    /// assert_eq!(args.single::<u32>().unwrap() * 2, 138);
    /// assert_eq!(args.single::<u32>().unwrap() * 2, 190);
    /// ```
    /// 
    #[inline]
    pub fn restore(&mut self) {
        self.offset = 0;
    }

    /// Like [`len`], but accounts quotes.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use serenity::framework::standard::Args;
    ///
    /// let mut args = Args::new(r#""42" "69""#, &[" ".to_string()]);
    ///
    /// assert_eq!(args.len_quoted(), 2); // `2` because `["42", "69"]`
    /// ```
    #[deprecated(since = "0.5.3", note = "Its task was merged with `len`, please use it instead.")]
    pub fn len_quoted(&mut self) -> usize {
        self.len()
    }

    /// "Skip" the argument (Sugar for `args.single::<String>().ok()`)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use serenity::framework::standard::Args;
    ///
    /// let mut args = Args::new("42 69", &[" ".to_string()]);
    ///
    /// args.skip();
    /// assert_eq!(args.single::<u32>().unwrap(), 69);
    /// ```
    pub fn skip(&mut self) -> Option<String> {
        if self.is_empty() {
            return None;
        }

        self.single::<String>().ok()
    }

    /// Like [`skip`], but do it multiple times.
    ///
    /// # Examples
    /// 
    /// ```rust
    /// use serenity::framework::standard::Args;
    ///
    /// let mut args = Args::new("42 69 88 99", &[" ".to_string()]);
    ///
    /// args.skip_for(3);
    /// assert_eq!(args.remaining(), 1);
    /// assert_eq!(args.single::<u32>().unwrap(), 99);
    /// ```
    ///
    /// [`skip`]: #method.skip
    pub fn skip_for(&mut self, i: u32) -> Option<Vec<String>> {
        if self.is_empty() {
            return None;
        }

        let mut vec = Vec::with_capacity(i as usize);

        for _ in 0..i {
            vec.push(self.skip()?);
        }

        Some(vec)
    }

    /// Like [`single`], but accounts quotes.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use serenity::framework::standard::Args;
    ///
    /// let mut args = Args::new(r#""42 69""#, &[" ".to_string()]);
    ///
    /// assert_eq!(args.single_quoted::<String>().unwrap(), "42 69");
    /// assert!(args.is_empty());
    /// ```
    ///
    /// [`single`]: #method.single
    pub fn single_quoted<T: FromStr>(&mut self) -> Result<T, T::Err>
        where T::Err: StdError {
        if self.is_empty() {
            return Err(Error::Eos);
        }

        let cur = &self.args[self.offset];

        let lit = if cur.kind == TokenKind::QuotedArgument {
            &cur.lit[1..cur.lit.len() - 1]
        } else {
            &cur.lit
        };

        let parsed = T::from_str(&lit)?;
        self.offset += 1;
        Ok(parsed)
    }

    /// Like [`single_quoted`], but doesn't advance.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use serenity::framework::standard::Args;
    ///
    /// let mut args = Args::new(r#""42 69""#, &[" ".to_string()]);
    ///
    /// assert_eq!(args.single_quoted_n::<String>().unwrap(), "42 69");
    /// assert_eq!(args.full(), r#""42 69""#);
    /// ```
    ///
    /// [`single_quoted`]: #method.single_quoted
    pub fn single_quoted_n<T: FromStr>(&self) -> Result<T, T::Err>
        where T::Err: StdError {
        if self.is_empty() {
            return Err(Error::Eos);
        }

        let cur = &self.args[self.offset];

        let lit = if cur.kind == TokenKind::QuotedArgument {
            &cur.lit[1..cur.lit.len() - 1]
        } else {
            &cur.lit
        };

        Ok(T::from_str(&lit)?)
    }

    /// Like [`multiple`], but accounts quotes.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use serenity::framework::standard::Args;
    ///
    /// let mut args = Args::new(r#""42" "69""#, &[" ".to_string()]);
    ///
    /// assert_eq!(*args.multiple_quoted::<u32>().unwrap(), [42, 69]);
    /// ```
    ///
    /// [`multiple`]: #method.multiple
    pub fn multiple_quoted<T: FromStr>(mut self) -> Result<Vec<T>, T::Err>
        where T::Err: StdError {
        if self.is_empty() {
            return Err(Error::Eos);
        }
        
        self.iter_quoted::<T>().collect()
    }

    /// Like [`iter`], but accounts quotes.
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use serenity::framework::standard::Args;
    ///
    /// let mut args = Args::new(r#""2" "5""#, &[" ".to_string()]);
    /// 
    /// assert_eq!(*args.iter_quoted::<u32>().map(|n| n.unwrap().pow(2)).collect::<Vec<_>>(), [4, 25]);
    /// assert!(args.is_empty());
    /// ```
    /// 
    /// [`iter`]: #method.iter
    pub fn iter_quoted<T: FromStr>(&mut self) -> IterQuoted<T>
        where T::Err: StdError {
        IterQuoted::new(self)
    }

    /// Parses all of the remaining arguments and returns them in a `Vec` (Sugar for `args.iter().collect::<Vec<_>>()`).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use serenity::framework::standard::Args;
    ///
    /// let args = Args::new("42 69", &[" ".to_string()]);
    ///
    /// assert_eq!(*args.multiple::<u32>().unwrap(), [42, 69]);
    /// ```
    pub fn multiple<T: FromStr>(mut self) -> Result<Vec<T>, T::Err>
        where T::Err: StdError {
        if self.is_empty() {
            return Err(Error::Eos);
        }

        self.iter::<T>().collect()
    }

    /// Provides an iterator that will spew arguments until the end of the message.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use serenity::framework::standard::Args;
    ///
    /// let mut args = Args::new("3 4", &[" ".to_string()]);
    ///
    /// assert_eq!(*args.iter::<u32>().map(|num| num.unwrap().pow(2)).collect::<Vec<_>>(), [9, 16]);
    /// assert!(args.is_empty());
    /// ```
    pub fn iter<T: FromStr>(&mut self) -> Iter<T> 
        where T::Err: StdError {
        Iter::new(self)
    }

    /// Returns the first argument that can be parsed and removes it from the message. The suitable argument 
    /// can be in an arbitrary position in the message.
    ///
    /// **Note**: 
    /// Unlike how other methods on this struct work, 
    /// this function permantently removes the argument if it was **found** and was **succesfully** parsed.
    /// Hence, use this with caution.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use serenity::framework::standard::Args;
    ///
    /// let mut args = Args::new("c42 69", &[" ".to_string()]);
    ///
    /// assert_eq!(args.find::<u32>().unwrap(), 69);
    /// assert_eq!(args.single::<String>().unwrap(), "c42");
    /// assert!(args.is_empty());
    /// ```
    pub fn find<T: FromStr>(&mut self) -> Result<T, T::Err>
        where T::Err: StdError {
        if self.is_empty() {
            return Err(Error::Eos);
        }

        let pos = match self.args.iter().position(|s| s.lit.parse::<T>().is_ok()) {
            Some(p) => p,
            None => return Err(Error::Eos),
        };

        let parsed = T::from_str(&self.args[pos].lit)?;
        self.args.remove(pos);
        self.rewind();

        Ok(parsed)
    }

    /// Like [`find`], but does not remove it.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use serenity::framework::standard::Args;
    ///
    /// let mut args = Args::new("c42 69", &[" ".to_string()]);
    ///
    /// assert_eq!(args.find_n::<u32>().unwrap(), 69);
    /// 
    /// // The `69` is still here, so let's parse it again.
    /// assert_eq!(args.single::<String>().unwrap(), "c42");
    /// assert_eq!(args.single::<u32>().unwrap(), 69);
    /// assert!(args.is_empty());
    /// ```
    /// 
    /// [`find`]: #method.find
    pub fn find_n<T: FromStr>(&mut self) -> Result<T, T::Err>
        where T::Err: StdError {
        if self.is_empty() {
            return Err(Error::Eos);
        }

        let pos = match self.args.iter().position(|s| s.lit.parse::<T>().is_ok()) {
            Some(p) => p,
            None => return Err(Error::Eos),
        };

        Ok(T::from_str(&self.args[pos].lit)?)
    }
}

impl ::std::ops::Deref for Args {
    type Target = str;

    fn deref(&self) -> &Self::Target { self.full() }
}

impl PartialEq<str> for Args {
    fn eq(&self, other: &str) -> bool {
        self.message == other
    }
}

impl<'a> PartialEq<&'a str> for Args {
    fn eq(&self, other: &&'a str) -> bool {
        self.message == *other
    }
}

impl PartialEq for Args {
    fn eq(&self, other: &Self) -> bool {
        self.message == *other.message
    }
}

impl Eq for Args {}

use std::marker::PhantomData;

/// Parse each argument individually, as an iterator.
pub struct Iter<'a, T: FromStr> where T::Err: StdError {
    args: &'a mut Args,
    _marker: PhantomData<T>,
}

impl<'a, T: FromStr> Iter<'a, T> where T::Err: StdError {
    fn new(args: &'a mut Args) -> Self {
        Iter { args, _marker: PhantomData }
    }
}

impl<'a, T: FromStr> Iterator for Iter<'a, T> where T::Err: StdError  {
    type Item = Result<T, T::Err>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.args.is_empty() {
            None
        } else {
            Some(self.args.single::<T>())
        }
    }
}

/// Same as [`Iter`], but considers quotes.
/// 
/// [`Iter`]: #struct.Iter.html
pub struct IterQuoted<'a, T: FromStr> where T::Err: StdError {
    args: &'a mut Args,
    _marker: PhantomData<T>,
}

impl<'a, T: FromStr> IterQuoted<'a, T> where T::Err: StdError {
    fn new(args: &'a mut Args) -> Self {
        IterQuoted { args, _marker: PhantomData }
    }
}

impl<'a, T: FromStr> Iterator for IterQuoted<'a, T> where T::Err: StdError  {
    type Item = Result<T, T::Err>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.args.is_empty() {
            None
        } else {
            Some(self.args.single_quoted::<T>())
        }
    }
}

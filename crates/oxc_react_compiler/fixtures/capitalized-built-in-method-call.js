// @validateNoCapitalizedCalls
function Component(props) {
  const daysInMonth = new Date(Date.UTC(props.year, props.month + 1, 0)).getUTCDate();
  const price = Intl.NumberFormat('en-US').format(props.price);
  return (
    <div>
      {daysInMonth} {price}
    </div>
  );
}

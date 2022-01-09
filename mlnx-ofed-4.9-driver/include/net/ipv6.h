#ifndef COMPAT_IPV6_H
#define COMPAT_IPV6_H

#include_next <net/ipv6.h>

/* Include the autogenerated header file */
#include "../../compat/config.h"

#ifndef HAVE_IPV6_ADDR_COPY
#define ipv6_addr_copy(a, b) (*(a) = *(b))
#endif

#ifndef HAVE_IP6_DST_HOPLIMIT
#define ip6_dst_hoplimit  LINUX_BACKPORT(ip6_dst_hoplimit)
int ip6_dst_hoplimit(struct dst_entry *dst);
#endif
#ifndef HAVE_IP4_DST_HOPLIMIT
#define ip4_dst_hoplimit  LINUX_BACKPORT(ip4_dst_hoplimit)
int ip4_dst_hoplimit(const struct dst_entry *dst);
#endif

#ifndef HAVE_IP6_MAKE_FLOWINFO
#define IPV6_TCLASS_SHIFT	20
static inline __be32 ip6_make_flowinfo(unsigned int tclass, __be32 flowlabel)
{
	return htonl(tclass << IPV6_TCLASS_SHIFT) | flowlabel;
}
#endif

#endif /* COMPAT_IPV6_H */